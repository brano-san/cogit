//! Open repositories, the event bus, credentials and the undo journal.
//!
//! The only crate in Cogit holding mutable global state. Everything else is pure
//! functions and short-lived handles.

use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::broadcast;

/// Event bus capacity. A slow subscriber lags rather than blocking the sender.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Opaque handle for a repository. Paths never cross the IPC boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, specta::Type)]
pub struct RepoId(pub u32);

/// Events pushed to the UI without it asking.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", content = "payload")]
pub enum AppEvent {
    RepoOpened {
        repo: RepoId,
    },
    RepoClosed {
        repo: RepoId,
    },
    RepoChanged {
        repo: RepoId,
        kind: fs_watcher::ChangeKind,
    },
    OperationStarted {
        id: u32,
        label: String,
    },
    OperationFinished {
        id: u32,
        success: bool,
    },
}

#[derive(Debug, Clone)]
pub struct OpenRepo {
    pub id: RepoId,
    pub root: PathBuf,
    pub display_name: String,
}

/// What the UI needs to show a repository the moment it is opened.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub repo: RepoId,
    /// Absolute path, with `/` separators so the UI never sees a backslash.
    pub root: String,
    /// Folder name, used as the label in the repository tree.
    pub name: String,
    pub is_bare: bool,
    pub head: git_engine::Head,
    pub branches: Vec<git_engine::Branch>,
}

/// Application-wide state, shared across every window.
#[derive(Debug)]
pub struct AppState {
    repos: RwLock<HashMap<RepoId, OpenRepo>>,
    next_repo_id: AtomicU32,
    events: broadcast::Sender<AppEvent>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            repos: RwLock::new(HashMap::new()),
            next_repo_id: AtomicU32::new(1),
            events,
        }
    }

    /// Subscribes to the event bus. Dropping the receiver is safe.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    /// Publishes an event. Having no subscribers is not an error — at startup there are none.
    pub fn emit(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }

    /// Opens the repository containing `path` and registers it.
    ///
    /// Blocking by design: `gix` reads the ref store synchronously. The Tauri layer
    /// wraps this in `spawn_blocking` so the runtime is never stalled
    /// (`doc/01-architecture.md` section 3).
    ///
    /// # Errors
    /// Propagates [`git_engine::GitError`] when the path encloses no repository or the
    /// ref store cannot be read. Nothing is registered in that case.
    pub fn open_repository(&self, path: &Path) -> Result<RepoSummary, git_engine::GitError> {
        let handle = git_engine::RepoHandle::open(path)?;
        let root = handle.root().to_path_buf();
        let head = handle.head()?;
        let branches = handle.branches()?;

        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );

        // Discovery collapses any path inside a project onto its root, so the same
        // repository opened from two different folders must reuse one entry.
        let id = self
            .find_by_root(&root)
            .unwrap_or_else(|| self.register(root.clone(), name.clone()));

        Ok(RepoSummary {
            repo: id,
            // IPC paths always use `/`, on every platform.
            root: root.to_string_lossy().replace('\\', "/"),
            name,
            is_bare: handle.is_bare(),
            head,
            branches,
        })
    }

    /// Finds an already open repository by its canonical root.
    #[must_use]
    pub fn find_by_root(&self, root: &Path) -> Option<RepoId> {
        self.repos
            .read()
            .values()
            .find(|r| r.root == root)
            .map(|r| r.id)
    }

    /// Registers a repository and returns its handle.
    pub fn register(&self, root: PathBuf, display_name: String) -> RepoId {
        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::Relaxed));
        // The guard is dropped before emitting so no lock is held across the send.
        self.repos.write().insert(
            id,
            OpenRepo {
                id,
                root,
                display_name,
            },
        );
        self.emit(AppEvent::RepoOpened { repo: id });
        id
    }

    pub fn unregister(&self, id: RepoId) -> bool {
        let removed = self.repos.write().remove(&id).is_some();
        if removed {
            self.emit(AppEvent::RepoClosed { repo: id });
        }
        removed
    }

    #[must_use]
    pub fn get(&self, id: RepoId) -> Option<OpenRepo> {
        self.repos.read().get(&id).cloned()
    }

    #[must_use]
    pub fn list(&self) -> Vec<OpenRepo> {
        let mut repos: Vec<OpenRepo> = self.repos.read().values().cloned().collect();
        repos.sort_by_key(|r| r.id);
        repos
    }
}

/// Convenience alias for the shared handle placed into Tauri via `.manage()`.
pub type SharedState = Arc<AppState>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_repositories_get_distinct_ids() {
        let state = AppState::new();
        let a = state.register(PathBuf::from("/a"), "a".into());
        let b = state.register(PathBuf::from("/b"), "b".into());
        assert_ne!(a, b);
        assert_eq!(state.list().len(), 2);
    }

    #[test]
    fn unregistering_an_unknown_repo_reports_false() {
        let state = AppState::new();
        assert!(!state.unregister(RepoId(999)));
    }

    #[test]
    fn subscribers_receive_lifecycle_events() {
        let state = AppState::new();
        let mut rx = state.subscribe();
        let id = state.register(PathBuf::from("/a"), "a".into());
        match rx.try_recv() {
            Ok(AppEvent::RepoOpened { repo }) => assert_eq!(repo, id),
            other => panic!("expected RepoOpened, got {other:?}"),
        }
    }

    #[test]
    fn emitting_without_subscribers_is_not_an_error() {
        // At startup nothing is listening yet; this must not panic or fail.
        let state = AppState::new();
        state.emit(AppEvent::OperationStarted {
            id: 1,
            label: "fetch".into(),
        });
    }

    #[test]
    fn listing_is_ordered_by_id() {
        let state = AppState::new();
        state.register(PathBuf::from("/a"), "a".into());
        state.register(PathBuf::from("/b"), "b".into());
        let ids: Vec<u32> = state.list().iter().map(|r| r.id.0).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
