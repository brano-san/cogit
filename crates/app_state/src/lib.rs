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

/// The Output panel is a recent history, not an audit log; the cap keeps a long session
/// from holding every byte Git ever printed.
pub const JOURNAL_CAPACITY: usize = 500;

/// What has to be put back to reverse one destructive operation (INV-12).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Recovery {
    Stash {
        oid: String,
    },
    Branch {
        name: String,
        oid: String,
    },
    /// Recorded for the journal, refused by undo: honesty beats a half-working restore.
    None,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SafetyEntry {
    pub id: u32,
    pub repo: RepoId,
    pub description: String,
    pub undoable: bool,
    pub recovery: Recovery,
}

/// Git reports mixed line endings, permissions and deprecated settings on `stderr` with
/// exit code 0. Nobody sees those unless we call them out.
#[must_use]
pub fn is_warning(entry: &git_engine::GitOutput) -> bool {
    entry.exit_code == Some(0) && !entry.stderr.trim().is_empty()
}

#[derive(Debug)]
pub struct AppState {
    repos: RwLock<HashMap<RepoId, OpenRepo>>,
    next_repo_id: AtomicU32,
    events: broadcast::Sender<AppEvent>,
    watchers: RwLock<HashMap<RepoId, fs_watcher::RepoWatcher>>,
    journal: Arc<RwLock<std::collections::VecDeque<git_engine::GitOutput>>>,
    safety: RwLock<Vec<SafetyEntry>>,
    next_entry_id: AtomicU32,
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
            watchers: RwLock::new(HashMap::new()),
            journal: Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(
                JOURNAL_CAPACITY,
            ))),
            safety: RwLock::new(Vec::new()),
            next_entry_id: AtomicU32::new(1),
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
        self.start_watching(id, &root, handle.git_dir());

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
        on_chunk: impl FnMut(GraphChunk) -> bool,
    ) -> Result<(), git_engine::GitError> {
        self.search_graph(
            repo,
            &git_engine::CommitQuery::default(),
            chunk_size,
            on_chunk,
        )
    }

    /// A filtered history is a flat list, not a graph: the parents of a match are usually
    /// filtered out, so lanes drawn between survivors would claim a lineage that is not
    /// there. Other clients do the same.
    pub fn search_graph(
        &self,
        repo: RepoId,
        query: &git_engine::CommitQuery,
        chunk_size: usize,
        mut on_chunk: impl FnMut(GraphChunk) -> bool,
    ) -> Result<(), git_engine::GitError> {
        let handle = self.handle(repo)?;
        let flat = !query.is_empty();
        let mut row = 0_u32;

        let mut cursor = graph_engine::LayoutCursor::default();
        let mut cancelled = false;
        let mut max_lane = 0_u16;

        handle.search_commits(query, chunk_size, |commits| {
            let (lanes, edges) = if flat {
                let lanes = commits
                    .iter()
                    .map(|_| {
                        let placement = graph_engine::LaneAssignment {
                            row,
                            lane: 0,
                            color: 0,
                            kind: graph_engine::NodeKind::Normal,
                        };
                        row += 1;
                        placement
                    })
                    .collect();
                (lanes, Vec::new())
            } else {
                let nodes: Vec<graph_engine::CommitNode> = commits
                    .iter()
                    .map(|c| graph_engine::CommitNode {
                        oid: c.oid.clone(),
                        parents: c.parents.clone(),
                    })
                    .collect();
                let placed = graph_engine::layout(&nodes, &mut cursor);
                max_lane = max_lane.max(placed.max_lane);
                (placed.lanes, placed.edges)
            };

            let keep = on_chunk(GraphChunk {
                commits,
                lanes,
                edges,
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

    pub fn diff_file(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
        options: &diff_engine::DiffOptions,
    ) -> Result<diff_engine::FileDiff, git_engine::GitError> {
        let (old, new) = self.handle(repo)?.diff_sides(spec, path)?;
        if old.is_none() && new.is_none() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{path} is absent from both sides of the diff"
            )));
        }

        let mut diff = diff_engine::diff_bytes(
            old.as_deref().unwrap_or_default(),
            new.as_deref().unwrap_or_default(),
            options,
        );
        if let diff_engine::FileDiff::Text { language, .. } = &mut diff {
            *language = diff_engine::language_for_path(path);
        }
        Ok(diff)
    }

    pub fn worktree_files(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::WorktreeFiles, git_engine::GitError> {
        self.handle(repo)?.worktree_files()
    }

    pub fn stage_paths(&self, repo: RepoId, paths: &[String]) -> Result<(), git_engine::GitError> {
        self.handle(repo)?.stage(paths)
    }

    pub fn unstage_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.handle(repo)?.unstage(paths)
    }

    /// Discarded work goes into a hidden stash first, so Undo has something to put back.
    pub fn discard_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        if paths.is_empty() {
            return Err(git_engine::GitError::InvalidState(
                "no paths given; refusing to act on the whole repository".to_owned(),
            ));
        }

        let handle = self.handle(repo)?;
        // A successful stash has already taken the changes out of the working tree, so
        // discarding again would only fail on paths Git no longer knows about.
        let stashed = handle
            .stash_paths(paths, &format!("cogit: discard {}", paths.join(", ")))
            .unwrap_or(None);
        if stashed.is_none() {
            handle.discard(paths)?;
        }

        self.record(
            repo,
            format!("Discard {}", paths.join(", ")),
            stashed.map_or(Recovery::None, |oid| Recovery::Stash { oid }),
        );
        Ok(())
    }

    pub fn commit(
        &self,
        repo: RepoId,
        request: &git_engine::CommitRequest,
    ) -> Result<String, git_engine::GitError> {
        self.handle(repo)?.commit(request)
    }

    pub fn checkout(
        &self,
        repo: RepoId,
        target: &git_engine::CheckoutTarget,
    ) -> Result<(), git_engine::GitError> {
        self.handle(repo)?.checkout(target)?;

        let what = match target {
            git_engine::CheckoutTarget::Branch { name } => name.clone(),
            git_engine::CheckoutTarget::Commit { oid } => oid.clone(),
        };
        self.record(repo, format!("Check out {what}"), Recovery::None);
        Ok(())
    }

    pub fn create_branch(
        &self,
        repo: RepoId,
        name: &str,
        start: Option<&str>,
        switch: bool,
    ) -> Result<(), git_engine::GitError> {
        self.handle(repo)?.create_branch(name, start, switch)
    }

    pub fn delete_branch(
        &self,
        repo: RepoId,
        name: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let handle = self.handle(repo)?;
        let oid = handle
            .branches()?
            .into_iter()
            .find(|branch| branch.name == name)
            .map(|branch| branch.oid);
        handle.delete_branch(name, force)?;

        self.record(
            repo,
            format!("Delete branch {name}"),
            oid.map_or(Recovery::None, |oid| Recovery::Branch {
                name: name.to_owned(),
                oid,
            }),
        );
        Ok(())
    }

    /// A repository is watched once; reopening the same path must not stack watchers.
    fn start_watching(&self, repo: RepoId, root: &Path, git_dir: &Path) {
        if self.watchers.read().contains_key(&repo) {
            return;
        }
        let events = self.events.clone();
        match fs_watcher::RepoWatcher::start(root, git_dir, move |change| {
            let _ = events.send(AppEvent::RepoChanged {
                repo,
                kind: change.kind,
            });
        }) {
            Ok(watcher) => {
                self.watchers.write().insert(repo, watcher);
            }
            Err(err) => {
                tracing::warn!(error = %err, repo = repo.0, "cannot watch the repository");
            }
        }
    }

    /// Held across a mutation so Cogit does not reload in response to its own writes.
    pub fn pause_watching(&self, repo: RepoId) {
        if let Some(watcher) = self.watchers.read().get(&repo) {
            watcher.pause();
        }
    }

    pub fn resume_watching(&self, repo: RepoId) {
        if let Some(watcher) = self.watchers.read().get(&repo) {
            watcher.resume();
        }
    }

    /// Newest first: the Output panel opens on what just happened.
    #[must_use]
    pub fn command_log(&self) -> Vec<git_engine::GitOutput> {
        self.journal.read().iter().rev().cloned().collect()
    }

    /// Just the count: the indicator refreshes often and the entries can be a megabyte each.
    #[must_use]
    pub fn command_problems(&self) -> u32 {
        let count = self
            .journal
            .read()
            .iter()
            .filter(|entry| entry.exit_code != Some(0) || is_warning(entry))
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    pub fn clear_command_log(&self) {
        self.journal.write().clear();
    }

    fn command_sink(&self) -> git_engine::CommandSink {
        let journal = Arc::clone(&self.journal);
        Arc::new(move |entry| {
            let mut log = journal.write();
            if log.len() == JOURNAL_CAPACITY {
                log.pop_front();
            }
            log.push_back(entry);
        })
    }

    /// Newest first, like the Output panel.
    #[must_use]
    pub fn safety_log(&self) -> Vec<SafetyEntry> {
        self.safety.read().iter().rev().cloned().collect()
    }

    pub fn undo_last(&self, repo: RepoId) -> Result<SafetyEntry, git_engine::GitError> {
        let entry = self
            .safety
            .read()
            .iter()
            .rev()
            .find(|entry| entry.repo == repo && entry.undoable)
            .cloned()
            .ok_or_else(|| git_engine::GitError::InvalidState("nothing to undo".to_owned()))?;

        let handle = self.handle(repo)?;
        match &entry.recovery {
            Recovery::Stash { oid } => handle.stash_apply(oid)?,
            Recovery::Branch { name, oid } => handle.create_branch(name, Some(oid), false)?,
            Recovery::None => {
                return Err(git_engine::GitError::InvalidState(
                    "this operation cannot be undone".to_owned(),
                ));
            }
        }

        self.safety.write().retain(|kept| kept.id != entry.id);
        Ok(entry)
    }

    fn record(&self, repo: RepoId, description: String, recovery: Recovery) {
        let entry = SafetyEntry {
            id: self.next_entry_id.fetch_add(1, Ordering::Relaxed),
            repo,
            description,
            undoable: !matches!(recovery, Recovery::None),
            recovery,
        };
        tracing::info!(
            repo = repo.0,
            entry = %entry.description,
            undoable = entry.undoable,
            "destructive operation recorded"
        );
        self.safety.write().push(entry);
    }

    fn handle(&self, repo: RepoId) -> Result<git_engine::RepoHandle, git_engine::GitError> {
        let open = self
            .get(repo)
            .ok_or_else(|| git_engine::GitError::RepoNotFound(format!("id {}", repo.0)))?;
        Ok(git_engine::RepoHandle::open(&open.root)?.with_journal(self.command_sink()))
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
