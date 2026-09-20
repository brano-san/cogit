use crate::{ChangeKind, DEBOUNCE_MS, RepoChanged, WatchError, classify_git_path, is_excluded};
use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub struct RepoWatcher {
    paused: Arc<AtomicBool>,
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
        let route = Route {
            root: root.to_path_buf(),
            git_dir: git_dir.to_path_buf(),
            paused: Arc::clone(&paused),
        };

        let mut debouncer = new_debouncer(
            Duration::from_millis(DEBOUNCE_MS),
            move |result: DebounceEventResult| {
                let Ok(events) = result else {
                    return;
                };
                for event in events {
                    if let Some(change) = route.classify(&event.path) {
                        on_change(change);
                    }
                }
            },
        )
        .map_err(|source| WatchError::Start {
            path: root.display().to_string(),
            source,
        })?;

        // The worktree is watched recursively because edits can happen anywhere in it;
        // the noise INV-06 warns about is filtered when routing, not by narrowing the
        // watch. `.git` is watched separately so `objects` can be skipped entirely.
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
            _debouncer: debouncer,
        })
    }

    /// Cogit's own mutations arrive as filesystem events too; reacting to them would
    /// reload the UI on top of the reload the mutation already triggered.
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
}

impl Route {
    fn classify(&self, path: &Path) -> Option<RepoChanged> {
        if self.paused.load(Ordering::Relaxed) {
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
