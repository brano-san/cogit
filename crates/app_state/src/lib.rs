mod avatars;
mod credentials;
mod diffing;
mod hooking;
pub mod logging;
mod network;
mod presets;
mod queue;
mod safety;
pub mod settings;
pub mod terminal;
mod worktrees;

pub use avatars::{Author, AvatarRow, Avatars};
pub use credentials::{
    KeyringStore, MemoryStore, SecretError, SecretStore, host_of, platform_store,
};
pub use presets::PresetStatus;
pub use queue::{Operation, OperationKind, OperationPermit, OperationPhase, Queue};
pub use safety::{Recovery, SafetyEntry};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

fn short(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

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
    /// Queued, started or finished — the phase is inside.
    Operation(Operation),
    AvatarReady {
        email: String,
    },
    /// A git command finished. The payload says what happened, not what was printed:
    /// a record can be two megabytes and most of them are never looked at.
    CommandRecorded(CommandNotice),
}

/// What the UI needs to decide whether to interrupt the user. The output itself is
/// fetched by `id` from the journal, and only when somebody asks to see it.
#[derive(Debug, Clone, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandNotice {
    pub id: u32,
    pub repo: String,
    pub operation: String,
    pub severity: git_engine::Severity,
    pub summary: String,
}

impl From<&git_engine::GitOutput> for CommandNotice {
    fn from(entry: &git_engine::GitOutput) -> Self {
        Self {
            id: entry.id,
            repo: entry.repo.clone(),
            operation: entry.operation.clone(),
            severity: entry.severity,
            summary: entry.summary.clone(),
        }
    }
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
    pub state: git_engine::RepoState,
    pub index_lock: Option<String>,
}

/// One hit from a folder scan. Paths cross IPC as strings, like every other path.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanHit {
    pub root: String,
    pub name: String,
    pub bare: bool,
    pub already_open: bool,
}

/// Commits arrive with their lane placement so the UI never computes layout (INV-02).
/// One row of the repository tree: enough to draw it without opening every repository.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoOverview {
    pub repo: RepoId,
    pub name: String,
    pub root: String,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    /// The folder is gone. The row stays so the user can remove it on purpose (T3.7).
    pub missing: bool,
}

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
const JOURNAL_CAPACITY: usize = 100;

/// Git reports mixed line endings, permissions and deprecated settings on `stderr` with
/// exit code 0. Nobody sees those unless we call them out.
#[must_use]
pub fn is_warning(entry: &git_engine::GitOutput) -> bool {
    entry.exit_code == Some(0) && !entry.stderr.trim().is_empty()
}

/// The journal is a ring: the oldest entry makes room for the newest. Free-standing so the
/// bound can be proven with a capacity of three instead of five hundred git processes.
pub fn record(
    log: &mut std::collections::VecDeque<git_engine::GitOutput>,
    capacity: usize,
    entry: git_engine::GitOutput,
) {
    while log.len() >= capacity.max(1) {
        log.pop_front();
    }
    log.push_back(entry);
}

pub struct AppState {
    repos: RwLock<HashMap<RepoId, OpenRepo>>,
    next_repo_id: AtomicU32,
    events: broadcast::Sender<AppEvent>,
    watchers: RwLock<HashMap<RepoId, fs_watcher::RepoWatcher>>,
    journal: Arc<RwLock<std::collections::VecDeque<git_engine::GitOutput>>>,
    safety: RwLock<Vec<safety::Undoable>>,
    next_entry_id: AtomicU32,
    secrets: Box<dyn SecretStore>,
    pictures: RwLock<Option<Avatars>>,
    preset_dir: RwLock<Option<std::path::PathBuf>>,
    /// The newest diff batch asked for per repository. An older answer never displaces
    /// a newer one, so responses cannot arrive out of order.
    pub(crate) newest_diff: RwLock<HashMap<RepoId, u32>>,
    /// One tree row per repository, good until the watcher or one of our own mutations
    /// says otherwise (problem 5).
    cached_rows: Arc<RwLock<HashMap<RepoId, RepoOverview>>>,
    rows_read: Arc<AtomicU32>,
    queue: Queue,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("repos", &self.repos)
            .finish_non_exhaustive()
    }
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
            secrets: platform_store(),
            pictures: RwLock::new(None),
            preset_dir: RwLock::new(None),
            newest_diff: RwLock::new(HashMap::new()),
            cached_rows: Arc::new(RwLock::new(HashMap::new())),
            rows_read: Arc::new(AtomicU32::new(0)),
            queue: Queue::default(),
        }
    }

    /// Tests and a machine without a credential store share this constructor.
    #[must_use]
    pub fn with_secrets(secrets: Box<dyn SecretStore>) -> Self {
        Self {
            secrets,
            ..Self::new()
        }
    }

    /// Turning avatars on is what creates the cache directory; `off` never gets here.
    pub fn enable_avatars(&self, dir: std::path::PathBuf) -> Result<(), ::avatars::CacheError> {
        self.enable_avatars_with(dir, std::sync::Arc::new(::avatars::Gravatar::new()))
    }

    pub fn enable_avatars_with<S: ::avatars::Source>(
        &self,
        dir: std::path::PathBuf,
        source: std::sync::Arc<S>,
    ) -> Result<(), ::avatars::CacheError> {
        let service = Avatars::new(dir, source, self.events.clone())?;
        *self.pictures.write() = Some(service);
        Ok(())
    }

    pub fn disable_avatars(&self) {
        *self.pictures.write() = None;
    }

    /// The addresses on screen, for the download queue. Reads nothing.
    pub fn avatar_window(&self, emails: &[String]) {
        if let Some(service) = self.pictures.read().as_ref() {
            service.window(emails);
        }
    }

    /// The pictures these authors have, if any. The answer is immediate.
    #[must_use]
    pub fn avatars(&self, authors: &[Author]) -> Vec<AvatarRow> {
        match self.pictures.read().as_ref() {
            Some(service) => service.rows(authors),
            None => avatars::rows_without_pictures(authors),
        }
    }

    /// Tests only: waits for the queue to settle so an assertion is not a race.
    pub fn drain_avatars(&self) {
        if let Some(service) = self.pictures.read().as_ref() {
            service.drain();
        }
    }

    /// Only ever answers whether a token exists: the value must not reach the webview.
    #[must_use]
    pub fn has_token(&self, host: &str) -> bool {
        self.secrets.get(host).is_some()
    }

    pub fn store_token(&self, host: &str, token: &str) -> Result<(), SecretError> {
        self.secrets.set(host, token)
    }

    pub fn forget_token(&self, host: &str) -> Result<(), SecretError> {
        self.secrets.delete(host)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    /// Brackets one operation with a start and a finish event, so the toolbar can show a
    /// spinner without every call site remembering to announce itself.
    pub fn tracked<T, E>(&self, label: &str, work: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        let mut operation = Operation {
            id: self.next_entry_id.fetch_add(1, Ordering::Relaxed),
            repo: None,
            kind: OperationKind::Other,
            label: label.to_owned(),
            phase: OperationPhase::Running,
            success: None,
        };
        self.emit(AppEvent::Operation(operation.clone()));

        let result = work();
        operation.phase = OperationPhase::Done;
        operation.success = Some(result.is_ok());
        self.emit(AppEvent::Operation(operation));
        result
    }

    pub fn emit(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }

    /// Repositories already open are marked, so the dialog can grey them out instead of
    /// offering to open them twice.
    pub fn scan_for_repositories(
        &self,
        root: &Path,
        max_depth: usize,
        mut on_found: impl FnMut(ScanHit) -> bool + Send,
    ) {
        let options = git_engine::discover::ScanOptions { max_depth };
        let mut wanted = true;
        git_engine::discover::scan(root, &options, |found| {
            if !wanted {
                return;
            }
            wanted = on_found(ScanHit {
                root: found.path.display().to_string(),
                name: found.name,
                bare: found.bare,
                already_open: self.find_by_root(&found.path).is_some(),
            });
        });
    }

    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`.
    pub fn open_repository(&self, path: &Path) -> Result<RepoSummary, git_engine::GitError> {
        let handle = git_engine::RepoHandle::open(path)?;
        let root = handle.root().to_path_buf();
        let head = handle.head()?;
        let branches = handle.branches()?;
        let tags = handle.tags()?;
        let status = handle.status()?;
        let state = handle.state()?;
        let index_lock = handle.index_lock();

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
            state,
            index_lock,
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
    /// there. Other clients do the same. Narrowing the visible refs is exempt — it drops
    /// whole tips, never a commit from inside a surviving lineage (R-51).
    pub fn search_graph(
        &self,
        repo: RepoId,
        query: &git_engine::CommitQuery,
        chunk_size: usize,
        mut on_chunk: impl FnMut(GraphChunk) -> bool,
    ) -> Result<(), git_engine::GitError> {
        let handle = self.handle(repo)?;
        let flat = query.filters_rows();
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
        self.quiet(repo);
        self.handle(repo)?.commit_details(rev)
    }

    pub fn commit_files(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<git_engine::FileEntry>, git_engine::GitError> {
        self.handle(repo)?.commit_files(rev)
    }

    pub fn worktree_files(
        &self,
        repo: RepoId,
        view: git_engine::WorktreeView,
    ) -> Result<git_engine::WorktreeFiles, git_engine::GitError> {
        self.handle(repo)?.worktree_files_with(view)
    }

    pub fn stage_paths(&self, repo: RepoId, paths: &[String]) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.stage(paths)
    }

    pub fn unstage_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
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

        self.quiet(repo);
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
        self.quiet(repo);
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
        self.quiet(repo);
        self.handle(repo)?.create_branch(name, start, switch)
    }

    pub fn rename_branch(
        &self,
        repo: RepoId,
        from: &str,
        to: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.rename_branch(from, to, force)
    }

    pub fn set_upstream(
        &self,
        repo: RepoId,
        branch: &str,
        upstream: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.set_upstream(branch, upstream)
    }

    /// Reaches the server, so it is tracked and journalled like any other network call.
    pub fn delete_remote_branch(
        &self,
        repo: RepoId,
        remote: &str,
        branch: &str,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.delete_remote_branch(remote, branch)
    }

    pub fn delete_branch(
        &self,
        repo: RepoId,
        name: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
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
        let rows = Arc::clone(&self.cached_rows);
        match fs_watcher::RepoWatcher::start(root, git_dir, move |change| {
            rows.write().remove(&repo);
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

    /// Called before every mutation: the UI reloads itself afterwards, so reacting to our
    /// own writes only makes it reload twice (doc/12-risks.md, R-25).
    fn quiet(&self, repo: RepoId) {
        // Our own writes are the one change the watcher will not report, so the row has to
        // be dropped here instead.
        self.forget_row(repo);
        if let Some(watcher) = self.watchers.read().get(&repo) {
            watcher.quiet_for(fs_watcher::DEFAULT_QUIET);
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
        let events = self.events.clone();
        Arc::new(move |entry| {
            let _ = events.send(AppEvent::CommandRecorded(CommandNotice::from(&entry)));
            record(&mut journal.write(), JOURNAL_CAPACITY, entry);
        })
    }

    /// One entry of the journal, by the number a notice carried. `None` once the ring
    /// has moved past it.
    #[must_use]
    pub fn command_outcome(&self, id: u32) -> Option<git_engine::GitOutput> {
        self.journal
            .read()
            .iter()
            .rev()
            .find(|entry| entry.id == id)
            .cloned()
    }

    /// Staging changes the status and nothing else; reopening the repository to learn
    /// that re-reads HEAD, every branch and every tag for no reason.
    pub fn repo_status(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::RepoStatus, git_engine::GitError> {
        self.handle(repo)?.status()
    }

    pub fn abort_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.abort_operation()?;
        self.record(
            repo,
            "Abort the operation in progress".to_owned(),
            Recovery::None,
        );
        Ok(())
    }

    pub fn continue_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.continue_operation()
    }

    pub fn stashes(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::StashEntry>, git_engine::GitError> {
        self.handle(repo)?.stashes()
    }

    /// Only the named paths, leaving everything else in the working tree (T5.3).
    pub fn stash_selection(
        &self,
        repo: RepoId,
        paths: &[String],
        message: &str,
    ) -> Result<(), git_engine::GitError> {
        if paths.is_empty() {
            return Err(git_engine::GitError::InvalidState(
                "no paths given; refusing to stash the whole repository".to_owned(),
            ));
        }
        self.quiet(repo);
        let handle = self.handle(repo)?;
        handle.stash_paths(paths, message).map(drop)
    }

    pub fn stash_contents(
        &self,
        repo: RepoId,
        index: u32,
    ) -> Result<git_engine::StashContents, git_engine::GitError> {
        self.handle(repo)?.stash_contents(index)
    }

    pub fn stash_push(
        &self,
        repo: RepoId,
        options: &git_engine::StashOptions,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.stash_push(options)
    }

    pub fn stash_apply(
        &self,
        repo: RepoId,
        index: u32,
        pop: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.stash_apply_index(index, pop)
    }

    pub fn stash_drop(&self, repo: RepoId, index: u32) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let oid = self.handle(repo)?.stash_drop(index)?;
        self.record(
            repo,
            format!("Drop stash@{{{index}}}"),
            Recovery::Stash { oid },
        );
        Ok(())
    }

    pub fn create_tag(
        &self,
        repo: RepoId,
        request: &git_engine::TagRequest,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.create_tag(request)
    }

    pub fn delete_tag(&self, repo: RepoId, name: &str) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let oid = handle
            .tags()?
            .into_iter()
            .find(|tag| tag.name == name)
            .map(|tag| tag.oid);
        handle.delete_tag(name)?;
        self.record(
            repo,
            format!("Delete tag {name}"),
            oid.map_or(Recovery::None, |oid| Recovery::Tag {
                name: name.to_owned(),
                oid,
            }),
        );
        Ok(())
    }

    /// Stashes first when the tree is dirty: restoring a past version must not quietly
    /// overwrite work in progress (doc/modules/M12-commit-surgery.md).
    pub fn rollback_to(
        &self,
        repo: RepoId,
        rev: &str,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let label = if paths.is_empty() {
            "the working tree".to_owned()
        } else {
            paths.join(", ")
        };
        let stashed = handle
            .stash_paths(paths, &format!("cogit: before rollback of {label}"))
            .unwrap_or(None);

        handle.rollback_to(rev, paths)?;
        self.record(
            repo,
            format!("Roll back {label} to {}", short(rev)),
            stashed.map_or(Recovery::None, |oid| Recovery::Stash { oid }),
        );
        Ok(())
    }

    /// Only the rows on screen: the whole history would be tens of thousands of tree
    /// comparisons for data nobody looks at (doc/modules/M13-commit-overlap.md).
    pub fn overlap_window(
        &self,
        repo: RepoId,
        base: &str,
        window: &[String],
    ) -> Result<Vec<git_engine::OverlapRow>, git_engine::GitError> {
        self.handle(repo)?.overlap_window(base, window)
    }

    pub fn rebase_progress(
        &self,
        repo: RepoId,
    ) -> Result<Option<git_engine::RebaseProgress>, git_engine::GitError> {
        self.handle(repo)?.rebase_progress()
    }

    pub fn rebase_todo(
        &self,
        repo: RepoId,
        base: &str,
    ) -> Result<Vec<git_engine::TodoEntry>, git_engine::GitError> {
        self.handle(repo)?.rebase_todo(base)
    }

    pub fn interactive_rebase(
        &self,
        repo: RepoId,
        base: &str,
        plan: &[git_engine::TodoEntry],
        paused: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        if paused {
            handle.interactive_rebase_paused(base, plan)?;
        } else {
            handle.interactive_rebase(base, plan)?;
        }

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Branch { name, oid },
            _ => Recovery::None,
        };
        self.record(
            repo,
            format!("Interactive rebase onto {}", short(base)),
            recovery,
        );
        Ok(())
    }

    pub fn flow_status(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::FlowStatus, git_engine::GitError> {
        self.handle(repo)?.flow_status()
    }

    pub fn flow_init(
        &self,
        repo: RepoId,
        config: &git_engine::FlowConfig,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.flow_init(config)
    }

    pub fn flow_start(
        &self,
        repo: RepoId,
        kind: git_engine::FlowKind,
        name: &str,
    ) -> Result<String, git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.flow_start(kind, name)
    }

    /// Destructive: the branch is deleted once it is folded back, so the head it had
    /// is kept for Undo.
    pub fn flow_finish(
        &self,
        repo: RepoId,
        kind: git_engine::FlowKind,
        name: &str,
        tag: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let status = handle.flow_status()?;
        let full = match kind {
            git_engine::FlowKind::Feature => format!("{}{name}", status.config.feature),
            git_engine::FlowKind::Release => format!("{}{name}", status.config.release),
            git_engine::FlowKind::Hotfix => format!("{}{name}", status.config.hotfix),
        };
        let oid = handle
            .run_git_reading(&["rev-parse", &full])
            .ok()
            .map(|out| out.stdout.trim().to_owned())
            .filter(|oid| !oid.is_empty());

        handle.flow_finish(kind, name, tag)?;

        self.record(
            repo,
            format!("Finish {full}"),
            oid.map_or(Recovery::None, |oid| Recovery::Branch { name: full, oid }),
        );
        Ok(())
    }

    /// The shared branches that already contain this commit; empty means safe to rewrite.
    pub fn protecting_refs(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.protecting_refs(rev)
    }

    pub fn is_published(&self, repo: RepoId, rev: &str) -> Result<bool, git_engine::GitError> {
        self.handle(repo)?.is_published(rev)
    }

    pub fn split_off(
        &self,
        repo: RepoId,
        rev: &str,
        paths: &[String],
        message: &str,
        split_first: bool,
    ) -> Result<(), git_engine::GitError> {
        let handle = self.handle(repo)?;
        let protecting = handle.protecting_refs(rev)?;
        if !protecting.is_empty() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{} is on {} — splitting it would rewrite what others already have",
                short(rev),
                protecting.join(", ")
            )));
        }

        self.quiet(repo);
        let before = handle.head()?;
        handle.split_off(rev, paths, message, split_first)?;

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Branch { name, oid },
            _ => Recovery::None,
        };
        self.record(
            repo,
            format!("Split {} off {}", paths.join(", "), short(rev)),
            recovery,
        );
        Ok(())
    }

    pub fn stage_mode(
        &self,
        repo: RepoId,
        path: &str,
        executable: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.stage_mode(path, executable)
    }

    pub fn merge(
        &self,
        repo: RepoId,
        options: &git_engine::MergeOptions,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        handle.merge(options)?;

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Branch { name, oid },
            _ => Recovery::None,
        };
        self.record(repo, format!("Merge {}", options.source), recovery);
        Ok(())
    }

    pub fn rebase(
        &self,
        repo: RepoId,
        options: &git_engine::RebaseOptions,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        handle.rebase(options)?;

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Branch { name, oid },
            _ => Recovery::None,
        };
        self.record(repo, format!("Rebase onto {}", options.onto), recovery);
        Ok(())
    }

    pub fn skip_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.skip_operation()
    }

    pub fn cherry_pick(
        &self,
        repo: RepoId,
        commits: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.replay(repo, commits, true)
    }

    pub fn revert(&self, repo: RepoId, commits: &[String]) -> Result<(), git_engine::GitError> {
        self.replay(repo, commits, false)
    }

    fn replay(
        &self,
        repo: RepoId,
        commits: &[String],
        pick: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        if pick {
            handle.cherry_pick(commits)?;
        } else {
            handle.revert(commits)?;
        }

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Branch { name, oid },
            _ => Recovery::None,
        };
        let verb = if pick { "Cherry-pick" } else { "Revert" };
        self.record(
            repo,
            format!("{verb} {} commit(s)", commits.len()),
            recovery,
        );
        Ok(())
    }

    pub fn reflog(
        &self,
        repo: RepoId,
        limit: u32,
    ) -> Result<Vec<git_engine::ReflogEntry>, git_engine::GitError> {
        self.handle(repo)?.reflog(limit as usize)
    }

    pub fn lost_commits(
        &self,
        repo: RepoId,
        limit: u32,
    ) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> {
        self.handle(repo)?.lost_commits(limit as usize)
    }

    /// Sorted by name so the tree does not reshuffle when a repository is reopened.
    #[must_use]
    pub fn overviews(&self) -> Vec<RepoOverview> {
        let mut rows: Vec<RepoOverview> = self
            .list()
            .into_iter()
            .map(|open| self.row_for(&open))
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Reading a row costs a `head`, a `branches` and a `status`; the panel asks for the
    /// whole tree on every refresh, and most rows have not moved since it last asked.
    fn row_for(&self, open: &OpenRepo) -> RepoOverview {
        if let Some(row) = self.cached_rows.read().get(&open.id) {
            return row.clone();
        }
        let row = self.overview_of(open);
        self.rows_read.fetch_add(1, Ordering::Relaxed);
        self.cached_rows.write().insert(open.id, row.clone());
        row
    }

    /// Whatever this repository's row said is no longer true.
    pub fn forget_row(&self, repo: RepoId) {
        self.cached_rows.write().remove(&repo);
    }

    /// How many rows were built from git rather than served from the last look.
    #[must_use]
    pub fn rows_read(&self) -> u32 {
        self.rows_read.load(Ordering::Relaxed)
    }

    pub fn close_repository(&self, repo: RepoId) -> bool {
        self.watchers.write().remove(&repo);
        self.newest_diff.write().remove(&repo);
        self.forget_row(repo);
        self.safety.write().retain(|held| held.entry.repo != repo);
        let removed = self.unregister(repo);
        if removed {
            self.emit(AppEvent::RepoClosed { repo });
        }
        removed
    }

    fn overview_of(&self, open: &OpenRepo) -> RepoOverview {
        let mut row = RepoOverview {
            repo: open.id,
            name: open.display_name.clone(),
            root: open.root.to_string_lossy().replace('\\', "/"),
            branch: None,
            ahead: 0,
            behind: 0,
            dirty: false,
            missing: false,
        };

        let Ok(handle) = git_engine::RepoHandle::open(&open.root) else {
            row.missing = true;
            return row;
        };
        if let Ok(git_engine::Head::Branch { name, .. }) = handle.head() {
            row.branch = Some(name);
        }
        if let Ok(branches) = handle.branches()
            && let Some(current) = branches.into_iter().find(|b| b.is_head)
        {
            row.ahead = current.ahead;
            row.behind = current.behind;
        }
        if let Ok(status) = handle.status() {
            row.dirty = !status.is_clean();
        }
        row
    }

    pub fn submodules(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::Submodule>, git_engine::GitError> {
        self.handle(repo)?.submodules()
    }

    /// The submodules directly under `parent`, which is empty for the top level.
    ///
    /// Lazy on purpose: a repository with nine submodules, each with its own, costs one
    /// walk per level, and the tree only needs the level the user opened (problem 5).
    pub fn submodules_under(
        &self,
        repo: RepoId,
        parent: &str,
    ) -> Result<Vec<git_engine::Submodule>, git_engine::GitError> {
        let handle = self.handle(repo)?;
        if parent.is_empty() {
            return handle.submodules();
        }
        git_engine::RepoHandle::open(&handle.root().join(parent))?.submodules()
    }

    /// Every path in the repository, tracked and untracked, never ignored.
    pub fn all_files(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.all_files()
    }

    /// Looks inside files, handing matches over in batches as they are found.
    pub fn search_contents(
        &self,
        repo: RepoId,
        request: &git_engine::SearchRequest<'_>,
        cancelled: &dyn Fn() -> bool,
        on_batch: &mut dyn FnMut(Vec<git_engine::ContentMatch>),
    ) -> Result<(), git_engine::GitError> {
        self.handle(repo)?
            .search_contents(request, cancelled, on_batch)
    }

    pub fn update_submodule(
        &self,
        repo: RepoId,
        path: &str,
        init: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.update_submodule(path, init)
    }

    pub fn blame(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
    ) -> Result<Vec<git_engine::BlameLine>, git_engine::GitError> {
        self.handle(repo)?.blame(path, rev)
    }

    pub fn remote_url(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self.handle(repo)?.remote_url(name))
    }

    pub fn add_to_gitignore(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.add_to_gitignore(paths)
    }

    pub fn delete_untracked(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.delete_untracked(paths)?;
        self.record(
            repo,
            format!("Delete {} untracked path(s)", paths.len()),
            Recovery::None,
        );
        Ok(())
    }

    pub fn conflicted_paths(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.conflicted_paths()
    }

    pub fn conflict_text(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<git_engine::ConflictText, git_engine::GitError> {
        Ok(self.handle(repo)?.conflict_sides(path)?.to_text())
    }

    pub fn resolve_conflict(
        &self,
        repo: RepoId,
        path: &str,
        side: git_engine::ConflictSide,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.resolve_with(path, side)
    }

    pub fn resolve_conflict_text(
        &self,
        repo: RepoId,
        path: &str,
        text: &str,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.resolve_with_text(path, text)
    }

    pub fn find(
        &self,
        repo: RepoId,
        query: &str,
        limit: u32,
    ) -> Result<Vec<git_engine::Found>, git_engine::GitError> {
        self.handle(repo)?.find(query, limit as usize)
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
        state.emit(AppEvent::Operation(Operation {
            id: 1,
            repo: None,
            kind: OperationKind::Fetch,
            label: "fetch".into(),
            phase: OperationPhase::Running,
            success: None,
        }));
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

/// What a batch diff came back with. A request the user has already moved on from stops
/// between files rather than finishing work nobody will look at.
#[allow(clippy::items_after_test_module)]
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiffBatch {
    Ready {
        files: Vec<diff_engine::FileDiffEntry>,
    },
    /// A newer request for the same repository started while this one was running.
    Superseded,
}

#[allow(clippy::items_after_test_module)]
impl AppState {
    /// Every commit that changed a fragment of a file, newest first.
    pub fn investigate(
        &self,
        repo: RepoId,
        path: &str,
        from: u32,
        to: u32,
        limit: u32,
    ) -> Result<Vec<git_engine::InvestigationStep>, git_engine::GitError> {
        self.handle(repo)?
            .investigate(path, from, to, limit as usize)
    }

    /// The file as it stood before a commit. Decoded lossily on purpose: a file the user
    /// cannot open at all is worse than one rendered oddly.
    pub fn file_before(
        &self,
        repo: RepoId,
        oid: &str,
        path: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self
            .handle(repo)?
            .file_before(oid, path)?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }
}
