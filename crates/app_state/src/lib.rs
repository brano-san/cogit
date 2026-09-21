mod avatars;
mod credentials;
pub mod logging;
mod presets;
pub mod terminal;

pub use avatars::{Author, AvatarRow, Avatars};
pub use credentials::{
    KeyringStore, MemoryStore, SecretError, SecretStore, host_of, platform_store,
};
pub use presets::PresetStatus;

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
    OperationStarted {
        id: u32,
        label: String,
    },
    OperationFinished {
        id: u32,
        success: bool,
    },
    AvatarReady {
        email: String,
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
    Tag {
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
    safety: RwLock<Vec<SafetyEntry>>,
    next_entry_id: AtomicU32,
    secrets: Box<dyn SecretStore>,
    pictures: RwLock<Option<Avatars>>,
    preset_dir: RwLock<Option<std::path::PathBuf>>,
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

    /// The authors visible right now: the answer is immediate, pictures catch up later.
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
        let id = self.next_entry_id.fetch_add(1, Ordering::Relaxed);
        self.emit(AppEvent::OperationStarted {
            id,
            label: label.to_owned(),
        });

        let result = work();
        self.emit(AppEvent::OperationFinished {
            id,
            success: result.is_ok(),
        });
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

        Ok(diff_engine::diff_one(
            path,
            old.as_deref().unwrap_or_default(),
            new.as_deref().unwrap_or_default(),
            options,
        ))
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
        self.tracked("Renaming branch", || {
            self.handle(repo)?.rename_branch(from, to, force)
        })
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
        self.tracked("Deleting remote branch", || {
            self.handle(repo)?.delete_remote_branch(remote, branch)
        })
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

    /// Called before every mutation: the UI reloads itself afterwards, so reacting to our
    /// own writes only makes it reload twice (doc/12-risks.md, R-25).
    fn quiet(&self, repo: RepoId) {
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
        Arc::new(move |entry| record(&mut journal.write(), JOURNAL_CAPACITY, entry))
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
        self.reverse(repo, entry)
    }

    /// Any entry, not only the newest: the recoveries are independent restores rather than
    /// a stack, so the order is the user's to choose (T5.7).
    pub fn undo_entry(&self, repo: RepoId, id: u32) -> Result<SafetyEntry, git_engine::GitError> {
        let entry = self
            .safety
            .read()
            .iter()
            .find(|entry| entry.id == id && entry.repo == repo && entry.undoable)
            .cloned()
            .ok_or_else(|| {
                git_engine::GitError::InvalidState(format!("no undoable entry {id} here"))
            })?;
        self.reverse(repo, entry)
    }

    fn reverse(
        &self,
        repo: RepoId,
        entry: SafetyEntry,
    ) -> Result<SafetyEntry, git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        match &entry.recovery {
            Recovery::Stash { oid } => handle.stash_apply(oid)?,
            Recovery::Branch { name, oid } => handle.create_branch(name, Some(oid), false)?,
            Recovery::Tag { name, oid } => handle.create_tag(&git_engine::TagRequest {
                name: name.clone(),
                target: Some(oid.clone()),
                message: None,
                force: false,
            })?,
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
        self.tracked("Stashing selection", || {
            handle.stash_paths(paths, message).map(drop)
        })
    }

    /// A failing check is a verdict the user reads, so it is tracked like any other run
    /// and never turned into an error that stops the rebase (T11.3).
    pub fn run_check(
        &self,
        repo: RepoId,
        command: &str,
    ) -> Result<git_engine::HookRun, git_engine::GitError> {
        self.handle(repo)?.run_check(command)
    }

    pub fn worktrees(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::WorktreeEntry>, git_engine::GitError> {
        self.handle(repo)?.worktrees()
    }

    /// Which worktree already has this branch, so a checkout can offer to go there instead
    /// of failing with `already checked out` (T3.8).
    pub fn worktree_holding(
        &self,
        repo: RepoId,
        branch: &str,
    ) -> Result<Option<git_engine::WorktreeEntry>, git_engine::GitError> {
        self.handle(repo)?.worktree_holding(branch)
    }

    pub fn add_worktree(
        &self,
        repo: RepoId,
        path: &str,
        branch: &str,
        create: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        self.tracked("Adding worktree", || {
            handle.add_worktree(path, branch, create)
        })
    }

    pub fn remove_worktree(
        &self,
        repo: RepoId,
        path: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.remove_worktree(path, force)
    }

    pub fn prune_worktrees(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.prune_worktrees()
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
        self.quiet(repo);
        let handle = self.handle(repo)?;
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

    pub fn hooks(&self, repo: RepoId) -> Result<git_engine::HookOverview, git_engine::GitError> {
        self.handle(repo)?.hooks()
    }

    /// Where the user's own presets live. Set once at startup from the app config dir.
    pub fn use_preset_dir(&self, dir: std::path::PathBuf) {
        *self.preset_dir.write() = Some(dir);
    }

    /// The catalogue told against one repository: tools resolved and config files checked.
    pub fn presets_for(&self, repo: RepoId) -> Result<Vec<PresetStatus>, git_engine::GitError> {
        let root = self
            .get(repo)
            .ok_or_else(|| git_engine::GitError::RepoNotFound(format!("id {}", repo.0)))?
            .root;

        let mut all: Vec<PresetStatus> = git_engine::builtin_presets()
            .into_iter()
            .map(|preset| presets::status_for(preset, &root, false))
            .collect();
        if let Some(dir) = self.preset_dir.read().clone() {
            all.extend(
                presets::user_presets(&dir)
                    .into_iter()
                    .map(|preset| presets::status_for(preset, &root, true)),
            );
        }
        Ok(all)
    }

    /// Saves the hook as it stands now as a preset the user can install elsewhere.
    pub fn export_preset(
        &self,
        repo: RepoId,
        hook: &str,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<(), git_engine::GitError> {
        if !presets::valid_id(id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is not a usable preset name"
            )));
        }
        if git_engine::builtin_presets().iter().any(|p| p.id == id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is the name of a built-in preset"
            )));
        }
        let dir = self.preset_dir()?;
        let script = self.handle(repo)?.read_hook(hook)?;
        presets::write_preset(&dir, id, name, hook, description, &script).map(drop)
    }

    pub fn remove_preset(&self, id: &str) -> Result<(), git_engine::GitError> {
        if !presets::valid_id(id) || git_engine::builtin_presets().iter().any(|p| p.id == id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is not a preset of yours"
            )));
        }
        std::fs::remove_file(self.preset_dir()?.join(format!("{id}.toml")))?;
        Ok(())
    }

    fn preset_dir(&self) -> Result<std::path::PathBuf, git_engine::GitError> {
        self.preset_dir.read().clone().ok_or_else(|| {
            git_engine::GitError::InvalidState("no directory for your own presets".into())
        })
    }

    /// Wiring up a team's hooks is two steps, and the second is the one people forget.
    pub fn adopt_hooks(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        self.use_hooks_path(repo, path)?;
        self.handle(repo)?.add_eol_rule(path)
    }

    pub fn install_preset(&self, repo: RepoId, id: &str) -> Result<(), git_engine::GitError> {
        let saved = self
            .preset_dir
            .read()
            .clone()
            .map(|dir| presets::user_presets(&dir))
            .unwrap_or_default();
        let preset = git_engine::builtin_presets()
            .into_iter()
            .chain(saved)
            .find(|preset| preset.id == id)
            .ok_or_else(|| {
                git_engine::GitError::InvalidState(format!("there is no preset {id}"))
            })?;
        self.quiet(repo);
        self.handle(repo)?.install_preset(&preset)
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

    pub fn commit_template(&self, repo: RepoId) -> Result<Option<String>, git_engine::GitError> {
        self.handle(repo)?.commit_template()
    }

    pub fn bypass_log(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::Bypass>, git_engine::GitError> {
        self.handle(repo)?.bypass_log()
    }

    pub fn read_hook(&self, repo: RepoId, name: &str) -> Result<String, git_engine::GitError> {
        self.handle(repo)?.read_hook(name)
    }

    pub fn write_hook(
        &self,
        repo: RepoId,
        name: &str,
        body: &str,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.write_hook(name, body)
    }

    pub fn set_hook_enabled(
        &self,
        repo: RepoId,
        name: &str,
        enabled: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.set_hook_enabled(name, enabled)
    }

    /// Wires a versioned hook directory up. Never automatic: a hooks path inside the tree
    /// turns repository content into code that runs on commit (doc/modules/M10-hooks.md).
    pub fn use_hooks_path(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?
            .run_git(&["config", "core.hooksPath", path])
            .map(drop)
    }

    pub fn run_hook(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<git_engine::HookRun, git_engine::GitError> {
        self.handle(repo)?.run_hook(name)
    }

    pub fn remotes(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.remotes()
    }

    pub fn fetch(
        &self,
        repo: RepoId,
        remote: &str,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        self.tracked("Fetching", || {
            handle.fetch(remote, token.as_deref(), on_line)
        })
    }

    pub fn pull(
        &self,
        repo: RepoId,
        remote: &str,
        ff_only: bool,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        self.tracked("Pulling", || {
            handle.pull(remote, ff_only, token.as_deref(), on_line)
        })
    }

    pub fn push(
        &self,
        repo: RepoId,
        remote: &str,
        force: bool,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        self.tracked("Pushing", || {
            handle.push(remote, None, force, token.as_deref(), on_line)
        })
    }

    /// Only for an HTTP remote: SSH already authenticates through the agent, and handing
    /// a token to an unknown host would leak it.
    fn token_for(&self, handle: &git_engine::RepoHandle, remote: &str) -> Option<String> {
        let url = handle.remote_url(remote)?;
        if !git_engine::wants_auth(&url) {
            return None;
        }
        self.secrets.get(&host_of(&url)?)
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
            .map(|open| self.overview_of(&open))
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    pub fn close_repository(&self, repo: RepoId) -> bool {
        self.watchers.write().remove(&repo);
        self.safety.write().retain(|entry| entry.repo != repo);
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

    pub fn update_submodule(
        &self,
        repo: RepoId,
        path: &str,
        init: bool,
    ) -> Result<(), git_engine::GitError> {
        self.quiet(repo);
        self.handle(repo)?.update_submodule(path, init)
    }

    /// Builds the patch and applies it in one step: the two halves must never drift
    /// apart, and a half-applied selection is exactly the damage R-04 warns about.
    pub fn stage_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
        reverse: bool,
    ) -> Result<(), git_engine::GitError> {
        let Some(patch) = diff_engine::build_patch(request) else {
            return Err(git_engine::GitError::InvalidState(
                "nothing selected".to_owned(),
            ));
        };
        self.quiet(repo);
        self.handle(repo)?.apply_patch(&patch, reverse)
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

    /// Both sides of an image as `data:` URLs; `None` on a side the file is absent from.
    pub fn image_sides(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
    ) -> Result<(Option<String>, Option<String>), git_engine::GitError> {
        let (old, new) = self.handle(repo)?.diff_sides(spec, path)?;
        let encode = |bytes: Option<Vec<u8>>| {
            bytes.and_then(|data| {
                diff_engine::image_mime(&data).map(|mime| diff_engine::data_url(mime, &data))
            })
        };
        Ok((encode(old), encode(new)))
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

    /// The three sides already merged: the view needs regions to count and step through,
    /// not a file with markers in it.
    pub fn merge_preview(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<Vec<diff_engine::Region>, git_engine::GitError> {
        let sides = self.handle(repo)?.conflict_sides(path)?.to_text();
        Ok(diff_engine::merge3(
            sides.base.as_deref().unwrap_or_default(),
            sides.ours.as_deref().unwrap_or_default(),
            sides.theirs.as_deref().unwrap_or_default(),
        ))
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

/// The newest batch request seen per repository, keyed by `AppState` instance.
///
/// This would naturally be a field of `AppState`, but the struct is declared above the
/// branch divider and this branch may only append an `impl` below it (R-102). Keying by
/// the instance address keeps parallel tests, which each build their own `AppState`,
/// from cancelling one another.
static NEWEST_DIFF_REQUEST: std::sync::OnceLock<RwLock<HashMap<(usize, RepoId), u32>>> =
    std::sync::OnceLock::new();

fn newest_diff_request() -> &'static RwLock<HashMap<(usize, RepoId), u32>> {
    NEWEST_DIFF_REQUEST.get_or_init(|| RwLock::new(HashMap::new()))
}

#[allow(clippy::items_after_test_module)]
impl AppState {
    /// Records `request` as the newest for `repo` and reports whether it still is. An
    /// older number never displaces a newer one, so responses cannot arrive out of order.
    fn claim_diff_request(&self, repo: RepoId, request: u32) -> bool {
        let key = (std::ptr::from_ref(self) as usize, repo);
        let mut newest = newest_diff_request().write();
        match newest.get(&key) {
            Some(&seen) if seen > request => false,
            _ => {
                newest.insert(key, request);
                true
            }
        }
    }

    fn diff_request_is_current(&self, repo: RepoId, request: u32) -> bool {
        let key = (std::ptr::from_ref(self) as usize, repo);
        newest_diff_request().read().get(&key) == Some(&request)
    }

    /// Every file of a commit in one call.
    ///
    /// The object reads stay sequential — a `gix` repository is not shared across threads
    /// — and only the diffing is parallel, which is where the time goes anyway
    /// (doc/08-diff-engine.md section 9).
    ///
    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`, and `rayon` must
    /// never be entered from an async task ([INV-01](doc/01-architecture.md)).
    pub fn diff_files(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        paths: &[String],
        options: &diff_engine::DiffOptions,
        request: u32,
    ) -> Result<DiffBatch, git_engine::GitError> {
        if !self.claim_diff_request(repo, request) {
            return Ok(DiffBatch::Superseded);
        }

        let handle = self.handle(repo)?;
        let mut inputs = Vec::with_capacity(paths.len());

        for path in paths {
            // Between files, never inside one: there is no way to interrupt `imara-diff`
            // part-way, and a single file is short enough that it does not matter.
            if !self.diff_request_is_current(repo, request) {
                return Ok(DiffBatch::Superseded);
            }

            let (old, new) = handle.diff_sides(spec, path)?;
            if old.is_none() && new.is_none() {
                return Err(git_engine::GitError::InvalidState(format!(
                    "{path} is absent from both sides of the diff"
                )));
            }
            inputs.push(diff_engine::FileInput {
                path: path.clone(),
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
            });
        }

        let files = diff_engine::diff_many(inputs, options);
        if self.diff_request_is_current(repo, request) {
            Ok(DiffBatch::Ready { files })
        } else {
            Ok(DiffBatch::Superseded)
        }
    }

    /// Throws the selected lines away in the working tree.
    ///
    /// Journalled with `Recovery::None`: a line-level snapshot has nowhere to live, and an
    /// undo that half works is worse than one that says no (R-106). That is why the view
    /// confirms every discard.
    pub fn discard_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
    ) -> Result<(), git_engine::GitError> {
        let Some(patch) = diff_engine::build_patch(request) else {
            return Err(git_engine::GitError::InvalidState(
                "nothing selected".to_owned(),
            ));
        };
        self.quiet(repo);
        self.handle(repo)?
            .apply_patch_to(&patch, true, git_engine::PatchTarget::WorkTree)?;
        self.record(
            repo,
            format!("Discard lines in {}", request.path),
            Recovery::None,
        );
        Ok(())
    }

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
