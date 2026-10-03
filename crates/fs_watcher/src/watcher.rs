use crate::{
    ChangeKind, DEBOUNCE_MS, RepoChanged, WatchError, classify_git_path, is_excluded,
    is_nested_git_noise,
};
use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Our own writes arrive one debounce after the command finished, so the window has to
/// outlive it (doc/12-risks.md, R-25).
pub const DEFAULT_QUIET: Duration = Duration::from_millis(DEBOUNCE_MS * 4);

pub struct RepoWatcher {
    paused: Arc<AtomicBool>,
    quiet_until: Arc<Mutex<Option<Instant>>>,
    held: Arc<AtomicUsize>,
    /// Why the working tree is not watched, when the watch of the root was refused.
    degraded: Option<String>,
    /// Dropping the watcher closes the channel, which ends the debounce thread.
    _watcher: RecommendedWatcher,
}

impl RepoWatcher {
    /// `common_dir` is where refs and config live: `git_dir` itself, except in a linked
    /// worktree.
    pub fn start(
        root: &Path,
        git_dir: &Path,
        common_dir: &Path,
        on_change: impl Fn(RepoChanged) + Send + 'static,
    ) -> Result<Self, WatchError> {
        Self::start_with(root, git_dir, common_dir, on_change, |watcher, root| {
            watch(watcher, root, RecursiveMode::Recursive)
        })
    }

    fn start_with(
        root: &Path,
        git_dir: &Path,
        common_dir: &Path,
        on_change: impl Fn(RepoChanged) + Send + 'static,
        watch_root: impl FnOnce(&mut RecommendedWatcher, &Path) -> Result<(), WatchError>,
    ) -> Result<Self, WatchError> {
        let paused = Arc::new(AtomicBool::new(false));
        let quiet_until = Arc::new(Mutex::new(None));
        let held = Arc::new(AtomicUsize::new(0));
        let route = Route {
            root: root.to_path_buf(),
            git_dir: git_dir.to_path_buf(),
            common_dir: common_dir.to_path_buf(),
            paused: Arc::clone(&paused),
            quiet_until: Arc::clone(&quiet_until),
            held: Arc::clone(&held),
        };

        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        // Not through symlinks: a link to `$HOME` or a data set would be walked as part of
        // the tree and eat the inotify budget (W-05).
        let mut debouncer = RecommendedWatcher::new(
            move |event| {
                let _ = tx.send(event);
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .map_err(|source| WatchError::Start {
            path: root.display().to_string(),
            source,
        })?;
        std::thread::Builder::new()
            .name("fs-watcher-debounce".into())
            .spawn(move || debounce(&rx, &route, &on_change))
            .map_err(|error| WatchError::Start {
                path: root.display().to_string(),
                source: notify::Error::io(error),
            })?;

        // The INV-06 noise is filtered when routing, not by narrowing the watch.
        //
        // A root that cannot be watched (the inotify limit, a folder nobody may read) must
        // not take the git directory down with it: refs, HEAD and the index are still heard.
        let degraded = watch_root(&mut debouncer, root).err().map(|error| {
            tracing::warn!(%error, root = %root.display(), "the working tree is not watched");
            error.to_string()
        });
        // Inside the root the recursive watch already hears the git directory; a submodule's
        // and a linked worktree's are elsewhere. A watch of its own inside the root would
        // hold a folder open there, and Windows refuses to rename or move a folder with a
        // handle open anywhere below it (R-438). Without the root watch they are needed.
        let outside = |path: &Path| degraded.is_some() || !path.starts_with(root);
        if outside(git_dir) {
            watch(&mut debouncer, git_dir, RecursiveMode::NonRecursive)?;
        }
        let linked = common_dir != git_dir;
        if linked && outside(common_dir) {
            // packed-refs and config sit at the top of the common directory.
            watch(&mut debouncer, common_dir, RecursiveMode::NonRecursive)?;
        }
        for name in crate::WATCHED_GIT_PATHS {
            let mut places = vec![git_dir.join(name)];
            if linked && matches!(*name, "refs" | "info") {
                places.push(common_dir.join(name));
            }
            for path in places
                .into_iter()
                .filter(|path| path.exists() && outside(path))
            {
                watch(&mut debouncer, &path, RecursiveMode::Recursive)?;
            }
        }
        let mut hooks = vec![git_dir.join("hooks")];
        if linked {
            hooks.push(common_dir.join("hooks"));
        }
        for path in hooks
            .into_iter()
            .filter(|path| path.is_dir() && outside(path))
        {
            watch(&mut debouncer, &path, RecursiveMode::Recursive)?;
        }

        Ok(Self {
            paused,
            quiet_until,
            held,
            degraded,
            _watcher: debouncer,
        })
    }

    /// The reason the working tree is not watched; only the git directory is.
    #[must_use]
    pub fn degraded(&self) -> Option<&str> {
        self.degraded.as_deref()
    }

    /// Only ever extends: a second mutation must not cut the first one's window short.
    pub fn quiet_for(&self, duration: Duration) {
        extend(&self.quiet_until, duration);
    }

    /// Quiet for as long as the guard lives, then for the debounce that trails the last
    /// write (R-445). A timer alone closed in the middle of a long git process.
    #[must_use]
    pub fn hold(&self) -> QuietHold {
        self.held.fetch_add(1, Ordering::SeqCst);
        QuietHold {
            held: Arc::clone(&self.held),
            quiet_until: Arc::clone(&self.quiet_until),
        }
    }

    /// Whether a quiet window is open right now.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        quiet(&self.held, &self.quiet_until)
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for RepoWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoWatcher")
            .field("paused", &self.paused.load(Ordering::Relaxed))
            .finish()
    }
}

/// One window per burst: the first event opens it, everything within `DEBOUNCE_MS` joins it.
///
/// Our own thin debounce because `notify-debouncer-mini` drops the event kind. notify asks
/// inotify for `IN_OPEN`, so every refresh reading `.git/HEAD` re-triggered itself forever
/// on Linux; only events that change something are kept.
///
/// The window closes at its deadline even when the channel still has a tail: `recv_timeout`
/// hands out a ready message before it looks at the clock. A long burst then yields a batch
/// every window instead of one at the very end.
fn debounce(
    rx: &mpsc::Receiver<notify::Result<Event>>,
    route: &Route,
    on_change: &impl Fn(RepoChanged),
) {
    let window = Duration::from_millis(DEBOUNCE_MS);
    while let Ok(first) = rx.recv() {
        let deadline = Instant::now() + window;
        let mut seen: Vec<(ChangeKind, PathBuf)> = Vec::new();
        let mut next = Some(first);
        while let Some(received) = next {
            route.absorb(received, &mut seen);
            let left = deadline.saturating_duration_since(Instant::now());
            next = if left.is_zero() {
                None
            } else {
                rx.recv_timeout(left).ok()
            };
        }
        for (kind, path) in seen {
            // The first path per kind per batch: a refresh that feeds itself shows here.
            tracing::info!(?kind, path = %path.display(), "watched path changed");
            on_change(RepoChanged { kind });
        }
    }
}

/// Opens and reads are not changes; a finished write is.
fn changes_something(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(access) => *access == AccessKind::Close(AccessMode::Write),
        _ => true,
    }
}

/// See [`RepoWatcher::hold`].
#[derive(Debug)]
pub struct QuietHold {
    held: Arc<AtomicUsize>,
    quiet_until: Arc<Mutex<Option<Instant>>>,
}

impl Drop for QuietHold {
    fn drop(&mut self) {
        extend(&self.quiet_until, DEFAULT_QUIET);
        self.held.fetch_sub(1, Ordering::SeqCst);
    }
}

fn extend(quiet_until: &Mutex<Option<Instant>>, duration: Duration) {
    let until = Instant::now() + duration;
    if let Ok(mut slot) = quiet_until.lock()
        && slot.is_none_or(|current| current < until)
    {
        *slot = Some(until);
    }
}

fn quiet(held: &AtomicUsize, quiet_until: &Mutex<Option<Instant>>) -> bool {
    held.load(Ordering::SeqCst) > 0
        || quiet_until
            .lock()
            .ok()
            .and_then(|slot| *slot)
            .is_some_and(|until| Instant::now() < until)
}

struct Route {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
    paused: Arc<AtomicBool>,
    quiet_until: Arc<Mutex<Option<Instant>>>,
    held: Arc<AtomicUsize>,
}

impl Route {
    fn is_quiet(&self) -> bool {
        quiet(&self.held, &self.quiet_until)
    }

    /// One debounce window, one event per kind.
    ///
    /// A single `git commit` rewrites the index, moves HEAD and touches a dozen refs; a
    /// `checkout` rewrites half the working tree. Announcing each path separately made the
    /// panel re-read the repository once per file, which is where the flicker on
    /// `dtv_device` came from (problem 5). Noise is dropped as events arrive, so a window
    /// keeps at most one entry per kind, however large the burst.
    fn absorb(&self, received: notify::Result<Event>, seen: &mut Vec<(ChangeKind, PathBuf)>) {
        // Judged when the event is taken off the channel, not when its window closes: a
        // stranger's write a moment before our own mutation must still get through.
        if self.paused.load(Ordering::Relaxed) || self.is_quiet() {
            return;
        }
        let event = match received {
            Ok(event) if event.need_rescan() => return everything(seen),
            Ok(event) if changes_something(&event.kind) => event,
            Ok(_) => return,
            // The OS queue overflowed (inotify says so with a flag, not an error): changes
            // were missed, and the panels are stale until the next event.
            Err(error) => {
                tracing::warn!(?error, "the file watcher lost events");
                return everything(seen);
            }
        };
        for path in event.paths {
            let Some(change) = self.classify(&path) else {
                continue;
            };
            // The file is also a working-tree change; this says the authors have to be
            // read again as well.
            let mailmap = (change.kind == ChangeKind::WorkingTree
                && path.strip_prefix(&self.root).ok() == Some(Path::new(".mailmap")))
            .then_some(ChangeKind::Mailmap);
            for kind in std::iter::once(change.kind).chain(mailmap) {
                if !seen.iter().any(|(known, _)| *known == kind) {
                    seen.push((kind, path.clone()));
                }
            }
        }
    }

    #[cfg(test)]
    fn coalesce(&self, paths: &[PathBuf]) -> Vec<RepoChanged> {
        let event = paths
            .iter()
            .fold(Event::new(EventKind::Any), |event, path| {
                event.add_path(path.clone())
            });
        let mut seen = Vec::new();
        self.absorb(Ok(event), &mut seen);
        seen.into_iter()
            .map(|(kind, _)| RepoChanged { kind })
            .collect()
    }

    fn classify(&self, path: &Path) -> Option<RepoChanged> {
        // The private directory first: in a linked worktree it lies inside the common one.
        let private = path.strip_prefix(&self.git_dir).ok();
        let shared = private
            .is_none()
            .then(|| path.strip_prefix(&self.common_dir).ok());
        let relative = if let Some(inside) = private.or(shared.flatten()) {
            let relative = to_slash(inside);
            if relative.starts_with("objects/") || relative == "objects" {
                return None;
            }
            // HEAD, the index and the operation markers at the top of the common folder
            // are the main worktree's, not this one's.
            if private.is_none() && !is_shared(&relative) {
                return None;
            }
            return classify_git_path(&relative).map(|kind| RepoChanged { kind });
        } else {
            path.strip_prefix(&self.root).ok()?
        };

        if is_excluded(relative) || is_nested_git_noise(relative) {
            return None;
        }
        Some(RepoChanged {
            kind: ChangeKind::WorkingTree,
        })
    }
}

/// What every worktree of a repository shares, at the top of the common folder.
fn is_shared(relative: &str) -> bool {
    relative == "refs"
        || relative.starts_with("refs/")
        || relative == "packed-refs"
        || relative == "config"
        || relative == "shallow"
        || relative == "hooks"
        || relative.starts_with("hooks/")
        || relative.starts_with("info/")
        || relative.starts_with("logs/refs/stash")
}

/// Events were lost, so anything may have moved.
fn everything(seen: &mut Vec<(ChangeKind, PathBuf)>) {
    for kind in [
        ChangeKind::Head,
        ChangeKind::Index,
        ChangeKind::Refs,
        ChangeKind::WorkingTree,
        ChangeKind::Stash,
        ChangeKind::Config,
    ] {
        if !seen.iter().any(|(known, _)| *known == kind) {
            seen.push((kind, PathBuf::new()));
        }
    }
}

fn to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn watch(
    debouncer: &mut RecommendedWatcher,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatchError> {
    debouncer
        .watch(path, mode)
        .map_err(|source| WatchError::Start {
            path: path.display().to_string(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChangeKind;

    fn route() -> Route {
        let root = PathBuf::from("C:/repo");
        Route {
            git_dir: root.join(".git"),
            common_dir: root.join(".git"),
            root,
            paused: Arc::new(AtomicBool::new(false)),
            quiet_until: Arc::new(Mutex::new(None)),
            held: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[test]
    fn a_burst_of_ref_writes_becomes_one_event() {
        let route = route();
        let burst: Vec<PathBuf> = (0..50)
            .map(|i| route.git_dir.join(format!("refs/heads/topic-{i}")))
            .collect();

        let batch = route.coalesce(&burst);

        assert_eq!(
            batch.len(),
            1,
            "one push must not redraw the tree fifty times"
        );
        assert_eq!(batch[0].kind, ChangeKind::Refs);
    }

    #[test]
    fn a_batch_still_carries_every_kind_it_saw() {
        let route = route();
        let paths = vec![
            route.git_dir.join("refs/heads/main"),
            route.git_dir.join("HEAD"),
            route.git_dir.join("index"),
            route.root.join("src/main.rs"),
            route.git_dir.join("refs/heads/other"),
        ];

        let batch = route.coalesce(&paths);
        let kinds: Vec<ChangeKind> = batch.iter().map(|change| change.kind).collect();

        assert_eq!(
            kinds.len(),
            4,
            "coalescing by kind, not by everything: {kinds:?}"
        );
        assert!(kinds.contains(&ChangeKind::Refs));
        assert!(kinds.contains(&ChangeKind::Head));
        assert!(kinds.contains(&ChangeKind::Index));
        assert!(kinds.contains(&ChangeKind::WorkingTree));
    }

    #[test]
    fn a_mailmap_at_the_root_is_a_working_tree_change_and_a_mailmap_one() {
        let route = route();

        let batch = route.coalesce(&[route.root.join(".mailmap")]);
        let kinds: Vec<ChangeKind> = batch.iter().map(|change| change.kind).collect();

        assert_eq!(kinds, [ChangeKind::WorkingTree, ChangeKind::Mailmap]);
    }

    #[test]
    fn a_mailmap_below_the_root_is_only_a_working_tree_change() {
        let route = route();

        let batch = route.coalesce(&[route.root.join("docs/.mailmap")]);
        let kinds: Vec<ChangeKind> = batch.iter().map(|change| change.kind).collect();

        assert_eq!(kinds, [ChangeKind::WorkingTree]);
    }

    #[test]
    fn the_order_a_batch_arrives_in_is_kept() {
        let route = route();
        let paths = vec![
            route.git_dir.join("index"),
            route.git_dir.join("HEAD"),
            route.git_dir.join("index"),
        ];

        let kinds: Vec<ChangeKind> = route.coalesce(&paths).iter().map(|c| c.kind).collect();

        assert_eq!(kinds, vec![ChangeKind::Index, ChangeKind::Head]);
    }

    #[test]
    fn noise_never_reaches_the_batch() {
        let route = route();
        let paths = vec![
            route.git_dir.join("objects/ab/cdef"),
            route.root.join("target/debug/cogit.exe"),
            route.root.join("node_modules/left-pad/index.js"),
        ];

        assert!(route.coalesce(&paths).is_empty());
    }

    #[test]
    fn a_refused_root_still_leaves_the_git_directory_watched() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git_dir = root.join(".git");
        std::fs::create_dir_all(git_dir.join("refs")).unwrap();
        std::fs::write(
            git_dir.join("HEAD"),
            "ref: refs/heads/main
",
        )
        .unwrap();
        let (tx, heard) = mpsc::channel();

        let watcher = RepoWatcher::start_with(
            root,
            &git_dir,
            &git_dir,
            move |change| {
                let _ = tx.send(change.kind);
            },
            |_, root| {
                Err(WatchError::Start {
                    path: root.display().to_string(),
                    source: notify::Error::new(notify::ErrorKind::MaxFilesWatch),
                })
            },
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        while heard.try_recv().is_ok() {}
        std::fs::write(
            git_dir.join("HEAD"),
            "ref: refs/heads/other
",
        )
        .unwrap();

        assert!(watcher.degraded().is_some());
        assert_eq!(
            heard.recv_timeout(Duration::from_secs(5)).ok(),
            Some(ChangeKind::Head)
        );
    }

    #[test]
    fn a_paused_watcher_produces_nothing() {
        let route = route();
        route.paused.store(true, Ordering::Relaxed);

        assert!(route.coalesce(&[route.git_dir.join("HEAD")]).is_empty());
    }

    #[test]
    fn a_lost_queue_marks_everything_as_changed() {
        let route = route();
        let overflow = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);

        for received in [Ok(overflow), Err(notify::Error::generic("lost"))] {
            let mut seen = Vec::new();
            route.absorb(received, &mut seen);
            let kinds: Vec<ChangeKind> = seen.iter().map(|(kind, _)| *kind).collect();
            for kind in [
                ChangeKind::Head,
                ChangeKind::Index,
                ChangeKind::Refs,
                ChangeKind::WorkingTree,
            ] {
                assert!(kinds.contains(&kind), "{kinds:?}");
            }
        }
    }

    #[test]
    fn a_lost_queue_is_silent_while_paused_or_quiet() {
        let route = route();
        route.paused.store(true, Ordering::Relaxed);
        let mut seen = Vec::new();
        route.absorb(Err(notify::Error::generic("lost")), &mut seen);
        assert!(seen.is_empty());

        route.paused.store(false, Ordering::Relaxed);
        extend(&route.quiet_until, Duration::from_secs(60));
        route.absorb(Err(notify::Error::generic("lost")), &mut seen);
        assert!(seen.is_empty());
    }

    #[test]
    fn an_event_taken_before_the_quiet_window_opened_is_kept() {
        let route = route();
        let mut seen = Vec::new();
        route.absorb(event(EventKind::Any, route.root.join("a.rs")), &mut seen);

        extend(&route.quiet_until, Duration::from_secs(60));
        route.absorb(event(EventKind::Any, route.git_dir.join("HEAD")), &mut seen);

        let kinds: Vec<ChangeKind> = seen.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(kinds, [ChangeKind::WorkingTree]);
    }

    // The main worktree's HEAD, index and merge state sit in the common folder a linked
    // worktree watches for packed-refs and config.
    #[test]
    fn a_linked_worktree_ignores_the_main_ones_head_and_index() {
        let common = PathBuf::from("C:/main/.git");
        let route = Route {
            root: PathBuf::from("C:/wt"),
            git_dir: common.join("worktrees/wt"),
            common_dir: common.clone(),
            paused: Arc::new(AtomicBool::new(false)),
            quiet_until: Arc::new(Mutex::new(None)),
            held: Arc::new(AtomicUsize::new(0)),
        };

        let main = [
            common.join("HEAD"),
            common.join("index"),
            common.join("MERGE_HEAD"),
        ];
        assert!(route.coalesce(&main).is_empty());
        let shared = route.coalesce(&[common.join("packed-refs")]);
        assert_eq!(shared[0].kind, ChangeKind::Refs);
        let own = route.coalesce(&[route.git_dir.join("HEAD")]);
        assert_eq!(own[0].kind, ChangeKind::Head);
    }

    fn event(kind: EventKind, path: PathBuf) -> notify::Result<Event> {
        Ok(Event::new(kind).add_path(path))
    }

    /// Runs `debounce` on a thread and reports each batch's kinds as it leaves.
    fn spawn_debounce(
        rx: mpsc::Receiver<notify::Result<Event>>,
    ) -> mpsc::Receiver<Vec<ChangeKind>> {
        let (out, batches) = mpsc::channel();
        std::thread::spawn(move || {
            let batch = Mutex::new(Vec::new());
            debounce(&rx, &route(), &|change: RepoChanged| {
                if let Ok(mut batch) = batch.lock() {
                    batch.push(change.kind);
                    let _ = out.send(batch.clone());
                }
            });
        });
        batches
    }

    #[test]
    fn a_hundred_thousand_noise_events_do_not_hold_the_head_back() {
        let route = route();
        let (tx, rx) = mpsc::channel();
        let create = EventKind::Create(notify::event::CreateKind::File);
        for i in 0..100_000 {
            let path = route.root.join(format!("node_modules/pkg-{i}/index.js"));
            tx.send(event(create, path)).unwrap();
        }
        tx.send(event(EventKind::Any, route.git_dir.join("HEAD")))
            .unwrap();
        drop(tx);

        let batches = spawn_debounce(rx);

        let kinds = batches.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(kinds, [ChangeKind::Head]);
    }

    #[test]
    fn a_window_closes_at_its_deadline_while_events_keep_coming() {
        let route = route();
        let (tx, rx) = mpsc::channel();
        let batches = spawn_debounce(rx);
        let started = Instant::now();
        let mut seen = 0;
        while started.elapsed() < Duration::from_millis(600) {
            tx.send(event(EventKind::Any, route.root.join("src/main.rs")))
                .unwrap();
            std::thread::sleep(Duration::from_millis(10));
            seen += batches.try_iter().count();
        }

        assert!(seen >= 3, "{seen} batches in 600 ms of steady writes");
    }
}
