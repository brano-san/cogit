mod avatars;
mod credentials;
pub mod desktop;
mod diffing;
pub mod environment;
mod file_actions;
mod flow;
mod graph_cache;
pub mod graph_overlay;
pub mod graph_wire;
mod handles;
mod hooking;
pub mod investigation;
pub mod licences;
pub mod logging;
mod network;
mod presets;
mod queue;
mod ref_ops;
mod remote_ops;
pub mod repo_rows;
mod rewrite;
mod safety;
pub mod settings;
mod stashing;
pub mod terminal;
mod worktrees;

pub use avatars::{Author, AvatarRow, Avatars};
pub use credentials::{
    KeyringStore, MemoryStore, SecretError, SecretStore, host_of, platform_store,
};
pub use graph_cache::{GraphProgress, GraphWindow};
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

/// The next event for a long-lived subscriber. One that fell behind skips what it missed
/// and carries on; only a closed bus ends it.
pub async fn next_event(events: &mut broadcast::Receiver<AppEvent>) -> Option<AppEvent> {
    loop {
        match events.recv().await {
            Ok(event) => return Some(event),
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                tracing::warn!(
                    missed,
                    "event subscriber fell behind; the missed events are skipped"
                );
            }
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

/// The confirmation promised Undo. Without the backup stash there is nothing to undo
/// with, so the destructive step is not taken at all (INV-12).
fn backup_failed(doing: &str, err: &git_engine::GitError) -> git_engine::GitError {
    tracing::error!(error = ?err, context = "backup stash before a destructive step");
    git_engine::GitError::InvalidState(format!(
        "Nothing was changed: the changes could not be saved for Undo before {doing} them. {err}"
    ))
}

fn short(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

/// For labels and stash messages: a message is on the command line too, so a thousand
/// names in it would cost the stash that Undo needs (R-191).
fn named(paths: &[String]) -> String {
    match paths {
        [_, _, _, _, ..] => format!("{}, {} and {} more", paths[0], paths[1], paths.len() - 2),
        _ => paths.join(", "),
    }
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
    /// Whether it appears in the Repositories panel as an entry of its own. A submodule
    /// reached by double-clicking its node is open and workable but not listed: it is
    /// already on screen, as a node of its parent (doc/12-risks.md, R-109).
    pub listed: bool,
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
    /// `cogit.tagGroupSeparator`, `/` when unset; read on every open, so a refresh sees a change.
    pub tag_group_separator: String,
}

/// What a commit moves besides the counters: the refs and the operation state (R-316).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoRefs {
    pub head: git_engine::Head,
    pub branches: Vec<git_engine::Branch>,
    pub tags: Vec<git_engine::Tag>,
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
    /// An operation stopped half way, or a detached HEAD: the row labels it (#22).
    pub state: git_engine::RepoState,
}

/// Tree rows by repository. A row read while its repository changed is not kept: the read
/// began before the change and may describe the state before it.
#[derive(Debug, Default)]
struct RowCache {
    rows: HashMap<RepoId, RepoOverview>,
    forgotten: u64,
}

impl RowCache {
    /// What `keep` needs to tell a read that raced a change.
    fn begin(&self) -> u64 {
        self.forgotten
    }

    fn keep(&mut self, since: u64, row: RepoOverview) {
        if self.forgotten == since {
            self.rows.insert(row.repo, row);
        }
    }

    fn forget(&mut self, repo: RepoId) {
        self.rows.remove(&repo);
        self.forgotten += 1;
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphChunk {
    pub commits: Vec<git_engine::CommitRow>,
    /// One per commit, in the same order: the node and every segment of its row. Cutting
    /// long links holds the last rows back, so a chunk can have fewer rows than commits.
    pub rows: Vec<graph_engine::GraphRow>,
    /// Folded merges whose count grew with this chunk.
    pub folds: Vec<graph_engine::Fold>,
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
    watchers: Arc<RwLock<HashMap<RepoId, fs_watcher::RepoWatcher>>>,
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
    cached_rows: Arc<RwLock<RowCache>>,
    rows_read: Arc<AtomicU32>,
    queue: Queue,
    /// One graph is on screen at a time; a newer request makes the walk before it stop.
    graph_generation: AtomicU32,
    graph: RwLock<graph_cache::GraphCache>,
    reachable: parking_lot::Mutex<HashMap<RepoId, git_engine::Reachable>>,
    handles: handles::HandleCache,
}

struct Quiet<'a> {
    state: &'a AppState,
    repo: RepoId,
}

impl Drop for Quiet<'_> {
    fn drop(&mut self) {
        self.state.silence(self.repo);
    }
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
            watchers: Arc::new(RwLock::new(HashMap::new())),
            journal: Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(
                JOURNAL_CAPACITY,
            ))),
            safety: RwLock::new(Vec::new()),
            next_entry_id: AtomicU32::new(1),
            secrets: platform_store(),
            pictures: RwLock::new(None),
            preset_dir: RwLock::new(None),
            newest_diff: RwLock::new(HashMap::new()),
            cached_rows: Arc::new(RwLock::new(RowCache::default())),
            rows_read: Arc::new(AtomicU32::new(0)),
            queue: Queue::default(),
            graph_generation: AtomicU32::new(0),
            graph: RwLock::new(graph_cache::GraphCache::default()),
            reachable: parking_lot::Mutex::new(HashMap::new()),
            handles: handles::HandleCache::default(),
        }
    }

    /// Ticking a hundred tags is one request, and ticking again mid-walk retires the old
    /// one: its next chunk sees it is no longer current and the walk ends there.
    pub fn begin_graph(&self) -> u32 {
        self.graph_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    #[must_use]
    pub fn is_current_graph(&self, generation: u32) -> bool {
        self.graph_generation.load(Ordering::SeqCst) == generation
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
        let old = self.pictures.write().replace(service);
        drop(old);
        Ok(())
    }

    /// Dropping the service waits for the downloads in flight, so it happens after the lock
    /// every avatar read takes is released, and never on an async worker (the command
    /// calls it through `blocking`).
    pub fn disable_avatars(&self) {
        let old = self.pictures.write().take();
        drop(old);
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
        git_engine::discover::scan_until(root, &options, |found| {
            on_found(ScanHit {
                root: found.path.display().to_string(),
                name: found.name,
                bare: found.bare,
                already_open: self.find_by_root(&found.path).is_some(),
            })
        });
    }

    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`.
    pub fn open_repository(&self, path: &Path) -> Result<RepoSummary, git_engine::GitError> {
        let mut watch = Steps::new();
        let handle = git_engine::RepoHandle::open(path)?;
        watch.done("open");
        self.open_with(handle, path, true, watch)
    }

    /// `open_repository` without the status, the registration and the watcher: after a
    /// commit only these moved, and the status is read by the refresh that follows.
    pub fn repo_refs(&self, repo: RepoId) -> Result<RepoRefs, git_engine::GitError> {
        let handle = self.handle(repo)?;
        Ok(RepoRefs {
            head: handle.head()?,
            branches: handle.branches()?,
            tags: handle.tags()?,
            state: handle.state()?,
            index_lock: handle.index_lock(),
        })
    }

    /// Opens a submodule from its node in the tree. `key` is the node's path from `owner`,
    /// the repository in the list — never from whichever submodule the panels show now.
    /// Already listed stays listed: asking for the same path by hand is a different request.
    pub fn open_submodule(
        &self,
        owner: RepoId,
        key: &str,
    ) -> Result<RepoSummary, git_engine::GitError> {
        let path = self.module_root(owner, key)?;
        let mut watch = Steps::new();
        let handle = git_engine::RepoHandle::open_exact(&path)?;
        watch.done("open");
        self.open_with(handle, &path, false, watch)
    }

    /// The one place a tree key becomes a directory, shared by listing and opening so the
    /// two can never disagree about where a node is (doc/12-risks.md, R-149).
    fn module_root(&self, owner: RepoId, key: &str) -> Result<PathBuf, git_engine::GitError> {
        Ok(self.handle(owner)?.root().join(key))
    }

    fn open_with(
        &self,
        handle: git_engine::RepoHandle,
        path: &Path,
        listed: bool,
        mut watch: Steps,
    ) -> Result<RepoSummary, git_engine::GitError> {
        // Timed step by step: the log of the three-monitor machine showed this command
        // taking 7.7 s on a repository with sixteen branches, and one number for the whole
        // thing does not say which read to go after (doc/12-risks.md, R-121).
        let root = handle.root().to_path_buf();
        let head = handle.head()?;
        watch.done("head");
        let branches = handle.branches()?;
        watch.done("branches");
        let tags = handle.tags()?;
        watch.done("tags");
        let status = handle.status()?;
        watch.done("status");
        let state = handle.state()?;
        watch.done("state");
        let index_lock = handle.index_lock();
        let tag_group_separator = handle.tag_group_separator();
        watch.report(path, branches.len());

        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );

        let id = self.find_or_register(root.clone(), name.clone(), listed);
        self.start_watching(id, &root, handle.git_dir(), handle.common_dir());

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
            tag_group_separator,
        })
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
    ) -> Result<Vec<git_engine::SkippedRef>, git_engine::GitError> {
        let handle = self.handle(repo)?;
        let flat = query.filters_rows();

        // Read once per load: a column that moved half way down would be worse than none.
        let mut cursor = graph_engine::LayoutCursor::with_mainline(mainline_of(&handle, query))
            .with_long_links(query.long_link_rows.unwrap_or(0));
        let mut cancelled = false;
        let mut view = graph_view(&handle, query, flat)?;

        // One order for the graph and the filtered list: by date, never a parent above a
        // child (R-162). A line to a parent the list will not show ends in an arrow (R-161).
        let on_commits = |mut commits: Vec<git_engine::CommitRow>| {
            if let Some(view) = view.as_mut() {
                commits.retain_mut(|c| view.admit(&c.oid, &mut c.parents));
            }
            let nodes: Vec<graph_engine::CommitNode> = commits
                .iter()
                .map(|c| graph_engine::CommitNode {
                    oid: c.oid.clone(),
                    parents: c.parents.clone(),
                    hidden: if flat {
                        c.parents
                            .iter()
                            .filter(|parent| !handle.shown_by(query, parent))
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    },
                })
                .collect();
            let rows = graph_engine::push(nodes, &mut cursor);
            let folds = view
                .as_mut()
                .map(graph_engine::ViewFilter::take_folds)
                .unwrap_or_default();

            let keep = on_chunk(GraphChunk {
                commits,
                rows,
                folds,
                is_last: false,
            });
            cancelled = !keep;
            keep
        };

        let skipped = handle.search_commits(query, chunk_size, on_commits)?;

        if !cancelled {
            // The rows held back to see how far their links reach (R-330).
            on_chunk(GraphChunk {
                commits: Vec::new(),
                rows: graph_engine::finish(&mut cursor),
                folds: view
                    .as_mut()
                    .map(graph_engine::ViewFilter::take_folds)
                    .unwrap_or_default(),
                is_last: true,
            });
        }
        Ok(skipped)
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

    pub fn worktree_files(
        &self,
        repo: RepoId,
        view: git_engine::WorktreeView,
    ) -> Result<git_engine::WorktreeFiles, git_engine::GitError> {
        self.handle(repo)?.worktree_files_with(view)
    }

    pub fn stage_paths(&self, repo: RepoId, paths: &[String]) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage(paths)
    }

    pub fn stage_all(&self, repo: RepoId, files: usize) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage_all(files)
    }

    pub fn unstage_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
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

        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        // A successful stash has already taken the changes out of the working tree, so
        // discarding again would only fail on paths Git no longer knows about.
        let stashed = handle
            .stash_paths(paths, &format!("cogit: discard {}", named(paths)))
            .map_err(|err| backup_failed("discarding", &err))?;
        if stashed.is_none() {
            handle.discard(paths)?;
        }

        self.record(
            repo,
            format!("Discard {}", named(paths)),
            stashed.map_or(Recovery::None, |oid| Recovery::Stash { oid }),
        );
        Ok(())
    }

    pub fn commit(
        &self,
        repo: RepoId,
        request: &git_engine::CommitRequest,
    ) -> Result<String, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.commit(request)
    }

    pub fn checkout(
        &self,
        repo: RepoId,
        target: &git_engine::CheckoutTarget,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
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
        let _quiet = self.quiet(repo);
        self.handle(repo)?.create_branch(name, start, switch)
    }

    pub fn rename_branch(
        &self,
        repo: RepoId,
        from: &str,
        to: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.rename_branch(from, to, force)
    }

    pub fn set_upstream(
        &self,
        repo: RepoId,
        branch: &str,
        upstream: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.set_upstream(branch, upstream)
    }

    /// Reaches the server, so it is tracked and journalled like any other network call.
    pub fn delete_remote_branch(
        &self,
        repo: RepoId,
        remote: &str,
        branch: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.delete_remote_branch(remote, branch)
    }

    pub fn delete_branch(
        &self,
        repo: RepoId,
        name: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
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
    fn start_watching(&self, repo: RepoId, root: &Path, git_dir: &Path, common_dir: &Path) {
        if self.watchers.read().contains_key(&repo) {
            return;
        }
        let events = self.events.clone();
        let rows = Arc::clone(&self.cached_rows);
        match fs_watcher::RepoWatcher::start(root, git_dir, common_dir, move |change| {
            rows.write().forget(repo);
            let _ = events.send(AppEvent::RepoChanged {
                repo,
                kind: change.kind,
            });
        }) {
            Ok(watcher) => {
                // Checked again under the lock: a second open of the same repository or a
                // close may have finished while this watcher was starting.
                let mut watchers = self.watchers.write();
                if watchers.contains_key(&repo) || !self.repos.read().contains_key(&repo) {
                    drop(watchers);
                    drop(watcher);
                } else {
                    watchers.insert(repo, watcher);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, repo = repo.0, "cannot watch the repository");
            }
        }
    }

    /// Called before every mutation: the UI reloads itself afterwards, so reacting to our
    /// own writes only makes it reload twice (doc/12-risks.md, R-25).
    /// Our own writes are the one change the watcher must not report: the UI reloads
    /// itself after a mutation. The window opens now and again when the guard drops, so a
    /// mutation that outlasts it does not echo either (R-197).
    #[must_use = "hold the guard until the mutation is done"]
    fn quiet(&self, repo: RepoId) -> Quiet<'_> {
        self.silence(repo);
        Quiet { state: self, repo }
    }

    fn silence(&self, repo: RepoId) {
        // The watcher will not report these writes, so the row has to be dropped here.
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

    /// Every git process of `repo` passes through here the moment it has exited. The quiet
    /// window is reopened from that moment, not only from the start and the end of the
    /// mutation: under load the process's last write can leave the debouncer before the
    /// mutation returns and the guard drops (R-197).
    fn command_sink(&self, repo: RepoId) -> git_engine::CommandSink {
        let journal = Arc::clone(&self.journal);
        let events = self.events.clone();
        let watchers = Arc::clone(&self.watchers);
        Arc::new(move |entry| {
            if let Some(watcher) = watchers.read().get(&repo) {
                watcher.quiet_for(fs_watcher::DEFAULT_QUIET);
            }
            let _ = events.send(AppEvent::CommandRecorded(CommandNotice::from(&entry)));
            record(&mut journal.write(), JOURNAL_CAPACITY, entry);
        })
    }

    /// Test seam: whether the watcher of `repo` is inside a quiet window right now.
    #[must_use]
    pub fn watcher_is_quiet(&self, repo: RepoId) -> bool {
        self.watchers
            .read()
            .get(&repo)
            .is_some_and(fs_watcher::RepoWatcher::is_quiet)
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

    /// `repo_status` and `conflicted_paths` in one read: after a mutation the cascade wants
    /// both (doc/12-risks.md, R-316).
    pub fn working_state(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::WorkingState, git_engine::GitError> {
        self.handle(repo)?.working_state()
    }

    pub fn create_tag(
        &self,
        repo: RepoId,
        request: &git_engine::TagRequest,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.create_tag(request)
    }

    pub fn delete_tag(&self, repo: RepoId, name: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let oid = handle.tag_target(name);
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

    pub fn stage_mode(
        &self,
        repo: RepoId,
        path: &str,
        executable: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage_mode(path, executable)
    }

    pub fn lost_commits(
        &self,
        repo: RepoId,
        limit: u32,
    ) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> {
        let handle = self.handle(repo)?;
        // Taken out, not held: a slow walk must not block another repository's refresh.
        let mut cache = self.reachable.lock().remove(&repo).unwrap_or_default();
        let lost = handle.lost_commits_with(limit as usize, &mut cache);
        self.reachable.lock().insert(repo, cache);
        lost
    }

    /// Sorted by name so the tree does not reshuffle when a repository is reopened.
    #[must_use]
    pub fn overviews(&self) -> Vec<RepoOverview> {
        let mut rows: Vec<RepoOverview> = self
            .list()
            .into_iter()
            .filter(|open| open.listed)
            .map(|open| self.row_for(&open))
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Reading a row costs a `head`, a `branches` and a `status`; the panel asks for the
    /// whole tree on every refresh, and most rows have not moved since it last asked.
    fn row_for(&self, open: &OpenRepo) -> RepoOverview {
        let since = {
            let cache = self.cached_rows.read();
            if let Some(row) = cache.rows.get(&open.id) {
                return row.clone();
            }
            cache.begin()
        };
        let row = self.overview_of(open);
        self.rows_read.fetch_add(1, Ordering::Relaxed);
        self.cached_rows.write().keep(since, row.clone());
        row
    }

    /// Whatever this repository's row said is no longer true.
    pub fn forget_row(&self, repo: RepoId) {
        self.cached_rows.write().forget(repo);
    }

    /// How many rows were built from git rather than served from the last look.
    #[must_use]
    pub fn rows_read(&self) -> u32 {
        self.rows_read.load(Ordering::Relaxed)
    }

    /// How many times a command had to open its repository from disk.
    #[must_use]
    pub fn repositories_opened(&self) -> u32 {
        self.handles.opened()
    }

    /// Open repositories kept for the next command.
    #[must_use]
    pub fn repositories_held(&self) -> usize {
        self.handles.held()
    }

    /// Closing froze the application with nothing in the log. Dropping a `RepoWatcher`
    /// joins the thread that delivers its events, and that was done **on the main thread
    /// and while holding the `watchers` lock** — so a delivery already in flight, which
    /// needs state this very call is holding, never finished (doc/12-risks.md, R-126).
    ///
    /// The watcher is now lifted out of the map, the guard is dropped, and only then is
    /// the watcher itself dropped. Every step says how long it took.
    pub fn close_repository(&self, repo: RepoId) -> bool {
        let mut watch = Steps::new();
        tracing::info!(repo = repo.0, "closing repository");

        let watcher = self.watchers.write().remove(&repo);
        watch.done("unhook-watcher");
        // Outside the lock, and said out loud: this is the join that used to hang.
        drop(watcher);
        watch.done("stop-watcher");

        self.newest_diff.write().remove(&repo);
        self.forget_row(repo);
        self.forget_graph(repo);
        self.reachable.lock().remove(&repo);
        watch.done("forget-state");

        let removed = self.unregister(repo);
        self.safety.write().retain(|held| held.entry.repo != repo);
        if removed {
            self.emit(AppEvent::RepoClosed { repo });
        }
        // An open of this repository that was starting its watcher as the first removal
        // ran has put one back by now, or sees the repository gone and drops it.
        let late = self.watchers.write().remove(&repo);
        drop(late);
        watch.done("unregister");
        watch.report_close(repo.0, removed);
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
            state: git_engine::RepoState::Clean,
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
        if let Ok(state) = handle.state() {
            row.state = state;
        }
        row
    }

    /// Resolved here on every call: the frontend names a scope, never a path to write.
    fn config_target(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
    ) -> Result<PathBuf, git_engine::GitError> {
        match scope {
            git_engine::ConfigScope::User => Ok(git_engine::user_config_path()),
            git_engine::ConfigScope::Repository => {
                let repo = repo.ok_or_else(|| {
                    git_engine::GitError::InvalidState("no repository is open".to_owned())
                })?;
                self.handle(repo)?.config_path()
            }
        }
    }

    pub fn config_file(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
    ) -> Result<git_engine::ConfigFile, git_engine::GitError> {
        git_engine::read_config(&self.config_target(repo, scope)?)
    }

    pub fn save_config_file(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
        text: &str,
        crlf: bool,
    ) -> Result<(), git_engine::GitError> {
        git_engine::save_config(&self.config_target(repo, scope)?, text, crlf)
    }

    /// The repository and every submodule below it; 72 ms on a tree of twenty-one.
    pub fn health(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::HealthFinding>, git_engine::GitError> {
        Ok(self.handle(repo)?.health_report())
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
        if parent.is_empty() {
            return self.handle(repo)?.submodules();
        }
        git_engine::RepoHandle::open_exact(&self.module_root(repo, parent)?)?.submodules()
    }

    pub fn tree_files(&self, repo: RepoId, rev: &str) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.tree_files(rev)
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
        let _quiet = self.quiet(repo);
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

    pub fn line_history(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
        line: u32,
        limit: usize,
    ) -> Result<Vec<git_engine::LineVersion>, git_engine::GitError> {
        self.handle(repo)?.line_history(path, rev, line, limit)
    }

    pub fn file_revisions(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
        limit: usize,
    ) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> {
        self.handle(repo)?.file_revisions(path, rev, limit)
    }

    pub fn remote_url(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self.handle(repo)?.remote_url(name))
    }

    pub fn ref_dates(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::RefDate>, git_engine::GitError> {
        self.handle(repo)?.ref_dates()
    }

    pub fn add_to_gitignore(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_to_gitignore(paths)
    }

    pub fn delete_untracked(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
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
        let _quiet = self.quiet(repo);
        self.handle(repo)?.resolve_with(path, side)
    }

    pub fn resolve_conflict_text(
        &self,
        repo: RepoId,
        path: &str,
        text: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
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
        Ok(self
            .handles
            .handle(repo, &open.root)?
            .with_journal(self.command_sink(repo)))
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
        self.register_as(root, display_name, true)
    }

    fn register_as(&self, root: PathBuf, display_name: String, listed: bool) -> RepoId {
        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::Relaxed));
        self.repos.write().insert(
            id,
            OpenRepo {
                id,
                root,
                display_name,
                listed,
            },
        );
        self.emit(AppEvent::RepoOpened { repo: id });
        id
    }

    /// Looked up and registered under one lock, so two opens of one path at once end up
    /// with one id. Asking for a listed repository lists it; the reverse never unlists.
    fn find_or_register(&self, root: PathBuf, display_name: String, listed: bool) -> RepoId {
        let mut repos = self.repos.write();
        if let Some(open) = repos.values_mut().find(|open| open.root == root) {
            open.listed |= listed;
            return open.id;
        }
        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::Relaxed));
        repos.insert(
            id,
            OpenRepo {
                id,
                root,
                display_name,
                listed,
            },
        );
        drop(repos);
        self.emit(AppEvent::RepoOpened { repo: id });
        id
    }

    pub fn unregister(&self, id: RepoId) -> bool {
        let removed = self.repos.write().remove(&id).is_some();
        self.handles.forget(id);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(repo: u32) -> RepoOverview {
        RepoOverview {
            repo: RepoId(repo),
            name: "a".into(),
            root: "/a".into(),
            branch: None,
            ahead: 0,
            behind: 0,
            dirty: false,
            missing: false,
            state: git_engine::RepoState::Clean,
        }
    }

    // The watcher dropped the row while the tree was reading it, and the read then put back
    // what it saw before the commit: the row stayed stale until the next change.
    #[test]
    fn a_row_read_across_a_change_is_not_kept() {
        let mut cache = RowCache::default();
        let since = cache.begin();
        cache.forget(RepoId(1));
        cache.keep(since, row(1));
        assert!(cache.rows.is_empty());
    }

    #[test]
    fn a_row_read_undisturbed_is_kept() {
        let mut cache = RowCache::default();
        let since = cache.begin();
        cache.keep(since, row(1));
        assert!(cache.rows.contains_key(&RepoId(1)));
    }

    // Close does not wait for the lane, so a reset still running when the repository was
    // closed recorded its Undo afterwards — for an id nothing would ever clear again.
    #[test]
    fn a_mutation_finishing_after_its_repository_closed_leaves_no_undo_entry() {
        let state = AppState::new();
        let id = state.register(PathBuf::from("/a"), "a".into());
        state.close_repository(id);

        state.record(id, "Hard reset".into(), safety::Recovery::None);

        assert!(state.safety_log().is_empty());
    }

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

/// HEAD, then `master`, then `main` — of those the graph draws. A primary ref that is
/// unticked or filtered out would hold column 0 empty for a line that never comes (R-161).
/// A filtered list is flat already; the view shapes a graph (#26).
fn graph_view(
    handle: &git_engine::RepoHandle,
    query: &git_engine::CommitQuery,
    flat: bool,
) -> Result<Option<graph_engine::ViewFilter>, git_engine::GitError> {
    let view = &query.view;
    if flat || !(view.first_parent || view.collapse_merged) {
        return Ok(None);
    }
    let roots = handle.walk_tips(query)?;
    Ok(Some(if view.first_parent {
        graph_engine::ViewFilter::first_parent(roots)
    } else {
        graph_engine::ViewFilter::collapse_merged(roots, view.expanded.iter().cloned())
    }))
}

fn mainline_of(handle: &git_engine::RepoHandle, query: &git_engine::CommitQuery) -> Option<String> {
    let ticked = |name: &str| {
        query
            .visible_refs
            .as_ref()
            .is_none_or(|refs| refs.iter().any(|rev| rev == name))
    };
    let branches = handle.branches().ok()?;
    let locals: Vec<(&str, &str)> = branches
        .iter()
        .filter(|branch| {
            branch.kind == git_engine::BranchKind::Local
                && ticked(&format!("refs/heads/{}", branch.name))
        })
        .map(|branch| (branch.name.as_str(), branch.oid.as_str()))
        .collect();
    // `head()`, not the branch marked as HEAD: a detached HEAD is a line worth keeping
    // straight too, and it belongs to no branch.
    let head = match handle.head().ok()? {
        git_engine::Head::Branch { oid, .. } | git_engine::Head::Detached { oid } => Some(oid),
        git_engine::Head::Unborn { .. } => None,
    }
    .filter(|_| ticked("HEAD"));
    let tip = graph_engine::mainline_tip(&locals, head.as_deref())?;
    (!query.filters_rows() || handle.shown_by(query, &tip)).then_some(tip)
}

/// Times the reads that make up one `open_repository` and writes them as one line.
struct Steps {
    started: std::time::Instant,
    last: std::time::Instant,
    parts: Vec<(&'static str, u128)>,
}

impl Steps {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            started: now,
            last: now,
            parts: Vec::new(),
        }
    }

    fn done(&mut self, what: &'static str) {
        let now = std::time::Instant::now();
        self.parts
            .push((what, now.duration_since(self.last).as_millis()));
        self.last = now;
    }

    fn report_close(&self, repo: u32, removed: bool) {
        let breakdown: Vec<String> = self
            .parts
            .iter()
            .map(|(what, ms)| format!("{what}={ms}ms"))
            .collect();
        tracing::info!(
            repo,
            removed,
            total_ms = self.started.elapsed().as_millis(),
            steps = %breakdown.join(" "),
            "repository closed"
        );
    }

    fn report(&self, path: &Path, branches: usize) {
        let total = self.started.elapsed().as_millis();
        let breakdown: Vec<String> = self
            .parts
            .iter()
            .map(|(what, ms)| format!("{what}={ms}ms"))
            .collect();
        tracing::info!(
            path = %path.display(),
            branches,
            total_ms = total,
            steps = %breakdown.join(" "),
            "repository read"
        );
    }
}
