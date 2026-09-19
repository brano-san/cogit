use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
pub struct RepoId(pub u32);

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

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub repo: RepoId,
    pub root: String,
    pub name: String,
    pub is_bare: bool,
    pub head: git_engine::Head,
    pub branches: Vec<git_engine::Branch>,
    pub tags: Vec<git_engine::Tag>,
    pub status: git_engine::RepoStatus,
}

/// Commits arrive with their lane placement so the UI never computes layout (INV-02).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphChunk {
    pub commits: Vec<git_engine::CommitRow>,
    pub lanes: Vec<graph_engine::LaneAssignment>,
    pub edges: Vec<graph_engine::GraphEdge>,
    pub max_lane: u16,
    pub is_last: bool,
}

pub const DEFAULT_CHUNK_SIZE: usize = 200;

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

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    pub fn emit(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }

    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`.
    pub fn open_repository(&self, path: &Path) -> Result<RepoSummary, git_engine::GitError> {
        let handle = git_engine::RepoHandle::open(path)?;
        let root = handle.root().to_path_buf();
        let head = handle.head()?;
        let branches = handle.branches()?;
        let tags = handle.tags()?;
        let status = handle.status()?;

        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );

        let id = self
            .find_by_root(&root)
            .unwrap_or_else(|| self.register(root.clone(), name.clone()));

        Ok(RepoSummary {
            repo: id,
            root: root.to_string_lossy().replace('\\', "/"),
            name,
            is_bare: handle.is_bare(),
            head,
            branches,
            tags,
            status,
        })
    }

    /// `on_chunk` returning `false` abandons the walk; no final chunk is sent.
    pub fn stream_graph(
        &self,
        repo: RepoId,
        chunk_size: usize,
        mut on_chunk: impl FnMut(GraphChunk) -> bool,
    ) -> Result<(), git_engine::GitError> {
        let open = self
            .get(repo)
            .ok_or_else(|| git_engine::GitError::RepoNotFound(format!("id {}", repo.0)))?;
        let handle = git_engine::RepoHandle::open(&open.root)?;

        let mut cursor = graph_engine::LayoutCursor::default();
        let mut cancelled = false;
        let mut max_lane = 0_u16;

        handle.stream_commits(chunk_size, |commits| {
            let nodes: Vec<graph_engine::CommitNode> = commits
                .iter()
                .map(|c| graph_engine::CommitNode {
                    oid: c.oid.clone(),
                    parents: c.parents.clone(),
                })
                .collect();
            let placed = graph_engine::layout(&nodes, &mut cursor);
            max_lane = max_lane.max(placed.max_lane);

            let keep = on_chunk(GraphChunk {
                commits,
                lanes: placed.lanes,
                edges: placed.edges,
                max_lane,
                is_last: false,
            });
            cancelled = !keep;
            keep
        })?;

        if !cancelled {
            on_chunk(GraphChunk {
                commits: Vec::new(),
                lanes: Vec::new(),
                edges: Vec::new(),
                max_lane,
                is_last: true,
            });
        }
        Ok(())
    }

    pub fn commit_details(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<git_engine::CommitDetails, git_engine::GitError> {
        self.handle(repo)?.commit_details(rev)
    }

    pub fn commit_files(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<git_engine::FileEntry>, git_engine::GitError> {
        self.handle(repo)?.commit_files(rev)
    }

    fn handle(&self, repo: RepoId) -> Result<git_engine::RepoHandle, git_engine::GitError> {
        let open = self
            .get(repo)
            .ok_or_else(|| git_engine::GitError::RepoNotFound(format!("id {}", repo.0)))?;
        git_engine::RepoHandle::open(&open.root)
    }

    #[must_use]
    pub fn find_by_root(&self, root: &Path) -> Option<RepoId> {
        self.repos
            .read()
            .values()
            .find(|r| r.root == root)
            .map(|r| r.id)
    }

    pub fn register(&self, root: PathBuf, display_name: String) -> RepoId {
        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::Relaxed));
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
