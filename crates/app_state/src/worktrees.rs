use crate::{AppState, Recovery, RepoId, RepoSummary, Steps};

impl AppState {
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

    /// Open in the panels, not listed: the Repositories tree stays as it is (R-184).
    pub fn open_worktree(
        &self,
        owner: RepoId,
        path: &str,
    ) -> Result<RepoSummary, git_engine::GitError> {
        let known = self
            .handle(owner)?
            .worktree_heads()?
            .into_iter()
            .any(|entry| entry.path == path && !entry.missing);
        if !known {
            return Err(git_engine::GitError::InvalidState(format!(
                "{path} is not an existing worktree of this repository"
            )));
        }
        let began = self.closes_so_far();
        let mut watch = Steps::new();
        let folder = std::path::Path::new(path);
        let handle = git_engine::RepoHandle::open_exact(folder)?;
        watch.done("open");
        self.open_with(handle, folder, Some(owner), watch, began)
    }

    pub fn add_worktree(
        &self,
        repo: RepoId,
        path: &str,
        branch: &git_engine::WorktreeBranch,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_worktree_with(path, branch)
    }

    pub fn check_revision(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<git_engine::RevisionCheck, git_engine::GitError> {
        self.handle(repo)?.check_revision(rev)
    }

    pub fn check_branch_name(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        self.handle(repo)?.check_branch_name(name)
    }

    /// `force` throws uncommitted work away, so it is stashed first and the journal
    /// can put it back (INV-12).
    pub fn remove_worktree(
        &self,
        repo: RepoId,
        path: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let name = std::path::Path::new(path).file_name().map_or_else(
            || path.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        // As `worktree_heads()` writes paths.
        let wanted = git_engine::slash_path(std::path::Path::new(path));
        let entry = handle
            .worktree_heads()?
            .into_iter()
            .find(|entry| entry.path == wanted);
        // Cogit passes one `--force`; git wants two for a locked worktree.
        if force && entry.as_ref().is_some_and(|entry| entry.locked.is_some()) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{name} is locked: unlock it first"
            )));
        }
        let checkout = entry.map(|entry| entry.branch.unwrap_or(entry.head));
        let stashed = if force {
            handle
                .stash_worktree_changes(path, &format!("cogit: before removing worktree {name}"))?
        } else {
            None
        };
        let removed = handle.remove_worktree(path, force);
        if removed.is_err()
            && let Some(stash) = &stashed
        {
            // Still registered: nothing was removed, so the work goes back where it was.
            let registered = handle
                .worktree_heads()?
                .into_iter()
                .any(|entry| entry.path == wanted && !entry.missing);
            if registered {
                git_engine::RepoHandle::open_exact(std::path::Path::new(path))?
                    .stash_apply(stash)?;
                handle.forget_backup(stash);
                return removed;
            }
        }
        let recovery = match (stashed, checkout) {
            (Some(stash), Some(checkout)) => Recovery::Worktree {
                path: path.to_owned(),
                checkout,
                stash,
            },
            _ => Recovery::None,
        };
        if removed.is_ok() || !matches!(recovery, Recovery::None) {
            self.record(repo, format!("Remove worktree {name}"), recovery);
        }
        removed
    }

    pub fn worktree_leftover(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<bool, git_engine::GitError> {
        Ok(self.handle(repo)?.worktree_leftover(path))
    }

    pub fn delete_worktree_leftover(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.delete_worktree_leftover(path)
    }

    pub fn prune_worktrees(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.prune_worktrees()
    }

    pub fn prune_worktree(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.prune_worktree(path)
    }

    pub fn repair_worktree(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.repair_worktree(path)
    }

    pub fn lock_worktree(
        &self,
        repo: RepoId,
        path: &str,
        reason: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.lock_worktree(path, reason)
    }

    pub fn unlock_worktree(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.unlock_worktree(path)
    }

    /// What removing a worktree would lose, read as three parallel reads (R-675). Each
    /// stage `send`s one chunk as it finishes, in whatever order; nothing after `stop`.
    pub fn scan_worktree_removal(
        &self,
        repo: RepoId,
        path: &str,
        stop: impl Fn() -> bool + Send + Sync + Clone + 'static,
        send: impl Fn(WorktreeScanChunk) + Sync,
    ) -> Result<(), git_engine::GitError> {
        let scan = self
            .handle(repo)?
            .worktree_scan(path)?
            .stopping_on(stop.clone());
        let started = std::time::Instant::now();
        let send = |stage: WorktreeScanStage,
                    answer: Result<WorktreeScanChunk, git_engine::GitError>| {
            tracing::info!(
                ?stage,
                elapsed = ?started.elapsed(),
                ok = answer.is_ok(),
                "worktree removal scan stage"
            );
            if !stop() {
                send(answer.unwrap_or_else(|error| WorktreeScanChunk::Failed { stage, error }));
            }
        };
        // Called from `spawn_blocking`, a thread of its own: plain scoped threads, no rayon.
        std::thread::scope(|scope| {
            let (send, scan) = (&send, &scan);
            scope.spawn(move || {
                let answer = scan
                    .changes()
                    .map(|files| WorktreeScanChunk::Changes { files });
                send(WorktreeScanStage::Changes, answer);
            });
            scope.spawn(move || {
                let answer = scan
                    .submodules()
                    .map(|modules| WorktreeScanChunk::Submodules { modules });
                send(WorktreeScanStage::Submodules, answer);
            });
            scope.spawn(move || {
                let answer = scan
                    .unpushed()
                    .map(|found| WorktreeScanChunk::Unpushed { found });
                send(WorktreeScanStage::Unpushed, answer);
            });
        });
        tracing::info!(elapsed = ?started.elapsed(), "worktree removal scan finished");
        Ok(())
    }
}

/// The three stages of the Remove Worktree scan, which one chunk reports as it finishes.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeScanStage {
    Changes,
    Submodules,
    Unpushed,
}

/// `Started` carries the id `cancel_operation` stops the scan by; each stage then answers
/// on its own, in whatever order they finish.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WorktreeScanChunk {
    Started {
        id: u32,
    },
    Changes {
        files: Vec<git_engine::FileEntry>,
    },
    Submodules {
        modules: git_engine::WorktreeSubmodules,
    },
    Unpushed {
        found: Vec<git_engine::UnpushedInSubmodule>,
    },
    Failed {
        stage: WorktreeScanStage,
        error: git_engine::GitError,
    },
    Done {
        cancelled: bool,
    },
}
