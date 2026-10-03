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
            && put_back(&handle, path, &wanted, stash)
        {
            return removed;
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

    pub fn move_worktree(
        &self,
        repo: RepoId,
        path: &str,
        to: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.move_worktree(path, to)
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

/// Still registered: nothing was removed, so the work goes back where it was. A failure
/// here keeps the backup and lets the caller write the Undo entry (INV-12).
fn put_back(handle: &git_engine::RepoHandle, path: &str, wanted: &str, stash: &str) -> bool {
    let restored = handle.worktree_heads().and_then(|heads| {
        let registered = heads
            .iter()
            .any(|entry| entry.path == wanted && !entry.missing);
        if registered {
            git_engine::RepoHandle::open_exact(std::path::Path::new(path))?.stash_apply(stash)?;
        }
        Ok(registered)
    });
    match restored {
        Ok(true) => {
            handle.forget_backup(stash);
            true
        }
        Ok(false) => false,
        Err(err) => {
            tracing::error!(error = ?err, context = "cannot put the stashed work back after a failed worktree removal");
            false
        }
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

#[cfg(test)]
mod tests {
    use super::put_back;

    // `put_back` runs on a worktree that is still registered; the stash is of its work.
    fn stashed() -> (
        test_fixtures::Fixture,
        git_engine::RepoHandle,
        String,
        String,
    ) {
        let f = test_fixtures::with_worktree().unwrap();
        let handle = git_engine::RepoHandle::open_exact(f.path()).unwrap();
        let path = handle
            .worktree_heads()
            .unwrap()
            .into_iter()
            .find(|entry| !entry.is_main)
            .unwrap()
            .path;
        std::fs::write(std::path::Path::new(&path).join("file0.txt"), "work\n").unwrap();
        let stash = handle
            .stash_worktree_changes(&path, "cogit: before removing worktree linked")
            .unwrap()
            .unwrap();
        (f, handle, path, stash)
    }

    fn backups(f: &test_fixtures::Fixture) -> String {
        f.git(&["for-each-ref", "--format=%(refname)", "refs/cogit/backup/"])
            .unwrap()
    }

    #[test]
    fn the_work_goes_back_and_its_backup_is_let_go() {
        let (f, handle, path, stash) = stashed();

        assert!(put_back(&handle, &path, &path, &stash));

        let file = std::path::Path::new(&path).join("file0.txt");
        assert_eq!(std::fs::read_to_string(file).unwrap(), "work\n");
        assert!(backups(&f).trim().is_empty());
    }

    // A second error must not leave the work only in the backup ref without an Undo entry:
    // the caller writes it when this says no.
    #[test]
    fn a_stash_that_cannot_be_applied_keeps_its_backup() {
        let (f, handle, path, stash) = stashed();
        let file = std::path::Path::new(&path).join("file0.txt");
        std::fs::write(&file, "newer\n").unwrap();

        assert!(!put_back(&handle, &path, &path, &stash));

        assert_eq!(std::fs::read_to_string(file).unwrap(), "newer\n");
        assert!(backups(&f).contains(&stash), "{}", backups(&f));
    }

    #[test]
    fn a_worktree_that_is_gone_keeps_its_backup() {
        let (f, handle, path, stash) = stashed();

        assert!(!put_back(&handle, &path, "elsewhere", &stash));

        assert!(backups(&f).contains(&stash));
    }
}
