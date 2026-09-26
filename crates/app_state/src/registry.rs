//! The open repositories: opening, registering and closing them, and the submodules below.

use crate::{AppEvent, AppState, RepoId};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub struct OpenRepo {
    pub id: RepoId,
    pub root: PathBuf,
    pub display_name: String,
    /// Whether it appears in the Repositories panel as an entry of its own. A submodule
    /// reached by double-clicking its node is open and workable but not listed: it is
    /// already on screen, as a node of its parent (doc/12-risks.md, R-109).
    pub listed: bool,
    /// The repository whose tree it was opened from, a submodule's or a worktree's; one
    /// that is not listed closes with it (R-508).
    pub owner: Option<RepoId>,
    /// The queue lane its writes wait in: the common git directory at the top of its owners,
    /// so a linked worktree and a submodule wait beside the repository they belong to.
    pub lane: PathBuf,
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

impl AppState {
    /// Repositories already open are marked, so the dialog can grey them out instead of
    /// offering to open them twice.
    pub fn scan_for_repositories(
        &self,
        root: &Path,
        max_depth: usize,
        cancelled: impl Fn() -> bool + Sync,
        mut on_found: impl FnMut(ScanHit) -> bool + Send,
    ) {
        let options = git_engine::discover::ScanOptions { max_depth };
        git_engine::discover::scan_cancellable(root, &options, cancelled, |found| {
            on_found(ScanHit {
                root: found.path.to_string_lossy().replace('\\', "/"),
                name: found.name,
                bare: found.bare,
                already_open: self.find_by_root(&found.path).is_some(),
            })
        });
    }

    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`.
    pub fn open_repository(&self, path: &Path) -> Result<RepoSummary, git_engine::GitError> {
        let began = self.closes_so_far();
        let mut watch = Steps::new();
        let handle = git_engine::RepoHandle::open(path)?;
        watch.done("open");
        self.open_with(handle, path, None, watch, began)
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
        let began = self.closes_so_far();
        let path = self.module_root(owner, key)?;
        let mut watch = Steps::new();
        let handle = git_engine::RepoHandle::open_exact(&path)?;
        watch.done("open");
        self.open_with(handle, &path, Some(owner), watch, began)
    }

    /// `open_repository` of one already open, by its id: listed or not stays as it was, so a
    /// re-read of a submodule or worktree on screen does not make it an entry (R-543).
    pub fn reread_repository(&self, repo: RepoId) -> Result<RepoSummary, git_engine::GitError> {
        let began = self.closes_so_far();
        let open = self.get(repo).ok_or_else(|| crate::not_open(repo))?;
        let mut watch = Steps::new();
        let handle = git_engine::RepoHandle::open_root(&open.root)?;
        watch.done("open");
        self.open_with(handle, &open.root, open.owner, watch, began)
    }

    /// The one place a tree key becomes a directory, shared by listing and opening so the
    /// two can never disagree about where a node is (doc/12-risks.md, R-149).
    fn module_root(&self, owner: RepoId, key: &str) -> Result<PathBuf, git_engine::GitError> {
        Ok(self.handle(owner)?.root().join(key))
    }

    /// `began`: `closes_so_far` before the first read. `owner`: `None` for an entry of
    /// the list, else the repository whose tree it was opened from.
    pub(crate) fn open_with(
        &self,
        handle: git_engine::RepoHandle,
        path: &Path,
        owner: Option<RepoId>,
        mut watch: Steps,
        began: u64,
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

        let common = handle.common_dir();
        let common = std::fs::canonicalize(common).unwrap_or_else(|_| common.to_path_buf());
        let Some(id) = self.find_or_register(root.clone(), name.clone(), owner, common, began)
        else {
            tracing::info!(root = %root.display(), "closed while it was opening; not registered");
            return Err(git_engine::GitError::InvalidState(format!(
                "{} was closed while it was opening",
                root.display()
            )));
        };
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

    pub fn update_submodule(
        &self,
        repo: RepoId,
        path: &str,
        init: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.update_submodule(path, init)
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

        // First, so that nothing begun from here on reaches the id, nor fills a cache
        // for it after it is cleared below.
        let removed = self.unregister(repo);
        self.safety.write().retain(|held| held.entry.repo != repo);
        watch.done("unregister");

        // A watcher an open was starting sees the repository gone and drops itself.
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
        watch.report_close(repo.0, removed);

        // Not one with work in its lane: its push or commit must not lose its registration.
        // The one on screen goes too: the panels let go of it first (R-543).
        let busy: Vec<Option<RepoId>> = self.queue.snapshot().iter().map(|op| op.repo).collect();
        let opened_from: Vec<RepoId> = self
            .repos
            .read()
            .values()
            .filter(|open| !open.listed && open.owner == Some(repo))
            .map(|open| open.id)
            .filter(|id| !busy.contains(&Some(*id)))
            .collect();
        for inner in opened_from {
            self.close_repository(inner);
        }
        removed
    }

    /// A repository closed meanwhile keeps a lane of its own, by its id.
    pub(crate) fn lane_of(&self, repo: RepoId) -> PathBuf {
        self.get(repo).map_or_else(
            || PathBuf::from(format!("<closed {}>", repo.0)),
            |open| open.lane,
        )
    }

    #[must_use]
    /// Listed in Repositories: a worktree or submodule open only in the panels is not.
    pub fn find_by_root(&self, root: &Path) -> Option<RepoId> {
        self.repos
            .read()
            .values()
            .find(|r| r.listed && r.root == root)
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
                lane: root.clone(),
                root,
                display_name,
                listed,
                owner: None,
            },
        );
        self.emit(AppEvent::RepoOpened { repo: id });
        id
    }

    /// How many closes there have been; an open takes it before its first read.
    pub(crate) fn closes_so_far(&self) -> u64 {
        self.closes.load(Ordering::SeqCst)
    }

    /// Looked up and registered under one lock, so two opens of one path at once end up
    /// with one id. Asking for a listed repository lists it; the reverse never unlists.
    /// `None` when `root` was closed after `began`: the open is older than the close.
    fn find_or_register(
        &self,
        root: PathBuf,
        display_name: String,
        owner: Option<RepoId>,
        common_dir: PathBuf,
        began: u64,
    ) -> Option<RepoId> {
        let listed = owner.is_none();
        let mut repos = self.repos.write();
        let lane = owner
            .and_then(|owner| repos.get(&owner))
            .map_or(common_dir, |owner| owner.lane.clone());
        if let Some(open) = repos.values_mut().find(|open| open.root == root) {
            open.listed |= listed;
            return Some(open.id);
        }
        if self
            .closed_at
            .lock()
            .get(&root)
            .is_some_and(|&at| at > began)
        {
            return None;
        }
        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::Relaxed));
        repos.insert(
            id,
            OpenRepo {
                id,
                root,
                display_name,
                listed,
                owner,
                lane,
            },
        );
        drop(repos);
        self.emit(AppEvent::RepoOpened { repo: id });
        Some(id)
    }

    pub fn unregister(&self, id: RepoId) -> bool {
        let removed = {
            let mut repos = self.repos.write();
            let gone = repos.remove(&id);
            if let Some(gone) = &gone {
                let at = self.closes.fetch_add(1, Ordering::SeqCst) + 1;
                self.closed_at.lock().insert(gone.root.clone(), at);
            }
            gone.is_some()
        };
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

/// Times the reads that make up one `open_repository` and writes them as one line.
pub(crate) struct Steps {
    started: std::time::Instant,
    last: std::time::Instant,
    parts: Vec<(&'static str, u128)>,
}

impl Steps {
    pub(crate) fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            started: now,
            last: now,
            parts: Vec::new(),
        }
    }

    pub(crate) fn done(&mut self, what: &'static str) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety;

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

    // A re-read of the open repository (open_repository by its root) running while it
    // closed registered it again under a new id, and the next list showed it open.
    #[test]
    fn an_open_begun_before_a_close_of_its_root_does_not_register_it_again() {
        let state = AppState::new();
        let root = PathBuf::from("/a");
        let id = state.register(root.clone(), "a".into());
        let began = state.closes_so_far();

        state.close_repository(id);

        assert_eq!(
            state.find_or_register(root.clone(), "a".into(), None, root.clone(), began),
            None
        );
        assert!(state.list().is_empty());
        let now = state.closes_so_far();
        assert!(
            state
                .find_or_register(root.clone(), "a".into(), None, root, now)
                .is_some()
        );
    }

    #[test]
    fn a_close_of_another_root_does_not_stop_an_open() {
        let state = AppState::new();
        let other = state.register(PathBuf::from("/b"), "b".into());
        let began = state.closes_so_far();

        state.close_repository(other);

        assert!(
            state
                .find_or_register(
                    PathBuf::from("/a"),
                    "a".into(),
                    None,
                    PathBuf::from("/a"),
                    began
                )
                .is_some()
        );
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
    fn listing_is_ordered_by_id() {
        let state = AppState::new();
        state.register(PathBuf::from("/a"), "a".into());
        state.register(PathBuf::from("/b"), "b".into());
        let ids: Vec<u32> = state.list().iter().map(|r| r.id.0).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
