use crate::{ChangeKind, DEBOUNCE_MS, RepoChanged, WatchError, classify_git_path, is_excluded};
use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Our own writes arrive one debounce after the command finished, so the window has to
/// outlive it (doc/12-risks.md, R-25).
pub const DEFAULT_QUIET: Duration = Duration::from_millis(DEBOUNCE_MS * 4);

pub struct RepoWatcher {
    paused: Arc<AtomicBool>,
    quiet_until: Arc<Mutex<Option<Instant>>>,
    /// Dropping the debouncer stops the background thread, so it has to be held.
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

impl RepoWatcher {
    pub fn start(
        root: &Path,
        git_dir: &Path,
        on_change: impl Fn(RepoChanged) + Send + 'static,
    ) -> Result<Self, WatchError> {
        let paused = Arc::new(AtomicBool::new(false));
        let quiet_until = Arc::new(Mutex::new(None));
        let route = Route {
            root: root.to_path_buf(),
            git_dir: git_dir.to_path_buf(),
            paused: Arc::clone(&paused),
            quiet_until: Arc::clone(&quiet_until),
        };

        let mut debouncer = new_debouncer(
            Duration::from_millis(DEBOUNCE_MS),
            move |result: DebounceEventResult| {
                let Ok(events) = result else {
                    return;
                };
                let paths: Vec<PathBuf> = events.into_iter().map(|event| event.path).collect();
                for change in route.coalesce(&paths) {
                    on_change(change);
                }
            },
        )
        .map_err(|source| WatchError::Start {
            path: root.display().to_string(),
            source,
        })?;

        // The INV-06 noise is filtered when routing, not by narrowing the watch.
        watch(&mut debouncer, root, RecursiveMode::Recursive)?;
        watch(&mut debouncer, git_dir, RecursiveMode::NonRecursive)?;
        for name in crate::WATCHED_GIT_PATHS {
            let path = git_dir.join(name);
            if path.exists() {
                watch(&mut debouncer, &path, RecursiveMode::Recursive)?;
            }
        }

        Ok(Self {
            paused,
            quiet_until,
            _debouncer: debouncer,
        })
    }

    /// Only ever extends: a second mutation must not cut the first one's window short.
    pub fn quiet_for(&self, duration: Duration) {
        let until = Instant::now() + duration;
        if let Ok(mut slot) = self.quiet_until.lock()
            && slot.is_none_or(|current| current < until)
        {
            *slot = Some(until);
        }
    }

    /// Whether a quiet window is open right now.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.quiet_until
            .lock()
            .ok()
            .and_then(|slot| *slot)
            .is_some_and(|until| Instant::now() < until)
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

struct Route {
    root: PathBuf,
    git_dir: PathBuf,
    paused: Arc<AtomicBool>,
    quiet_until: Arc<Mutex<Option<Instant>>>,
}

impl Route {
    fn is_quiet(&self) -> bool {
        self.quiet_until
            .lock()
            .ok()
            .and_then(|slot| *slot)
            .is_some_and(|until| Instant::now() < until)
    }

    /// One debounce window, one event per kind.
    ///
    /// A single `git commit` rewrites the index, moves HEAD and touches a dozen refs; a
    /// `checkout` rewrites half the working tree. Announcing each path separately made the
    /// panel re-read the repository once per file, which is where the flicker on
    /// `dtv_device` came from (problem 5). The debounce window is 100 ms, so this also caps
    /// the rate at ten updates per second per kind, however large the repository.
    fn coalesce(&self, paths: &[PathBuf]) -> Vec<RepoChanged> {
        let mut batch: Vec<RepoChanged> = Vec::new();
        for path in paths {
            let Some(change) = self.classify(path) else {
                continue;
            };
            if batch.iter().any(|seen| seen.kind == change.kind) {
                continue;
            }
            batch.push(change);
        }
        batch
    }

    fn classify(&self, path: &Path) -> Option<RepoChanged> {
        if self.paused.load(Ordering::Relaxed) || self.is_quiet() {
            return None;
        }

        let relative = if let Ok(inside) = path.strip_prefix(&self.git_dir) {
            let relative = to_slash(inside);
            if relative.starts_with("objects/") || relative == "objects" {
                return None;
            }
            return classify_git_path(&relative).map(|kind| RepoChanged {
                kind,
                path: relative,
            });
        } else {
            path.strip_prefix(&self.root).ok()?
        };

        if is_excluded(relative) {
            return None;
        }
        Some(RepoChanged {
            kind: ChangeKind::WorkingTree,
            path: to_slash(relative),
        })
    }
}

fn to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn watch(
    debouncer: &mut Debouncer<notify::RecommendedWatcher>,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatchError> {
    debouncer
        .watcher()
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
            root,
            paused: Arc::new(AtomicBool::new(false)),
            quiet_until: Arc::new(Mutex::new(None)),
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
    fn a_paused_watcher_produces_nothing() {
        let route = route();
        route.paused.store(true, Ordering::Relaxed);

        assert!(route.coalesce(&[route.git_dir.join("HEAD")]).is_empty());
    }
}
