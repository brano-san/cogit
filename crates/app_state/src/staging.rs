//! The working tree and the index: status, staging, discarding, the commit (M6).

use crate::{AppState, Recovery, RepoId, backup_failed, named};

impl AppState {
    pub(crate) fn local_worktree_files(
        &self,
        repo: RepoId,
        view: git_engine::WorktreeView,
    ) -> Result<std::sync::Arc<git_engine::WorktreeFiles>, git_engine::GitError> {
        if view == git_engine::WorktreeView::default() {
            return self.walked_for_list(repo);
        }
        self.handle(repo)?
            .worktree_files_with(view)
            .map(std::sync::Arc::new)
    }

    /// Staging changes the status and nothing else; reopening the repository to learn
    /// that re-reads HEAD, every branch and every tag for no reason.
    pub(crate) fn local_repo_status(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::RepoStatus, git_engine::GitError> {
        self.walked(repo).map(|files| files.state.status)
    }

    /// `repo_status` and `conflicted_paths` in one read: after a mutation the cascade wants
    /// both (doc/12-risks.md, R-316).
    pub(crate) fn local_working_state(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::WorkingState, git_engine::GitError> {
        self.walked(repo).map(|files| files.state.clone())
    }

    pub(crate) fn local_stage_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage(paths)
    }

    pub(crate) fn local_stage_all(
        &self,
        repo: RepoId,
        files: usize,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage_all(files)
    }

    pub(crate) fn local_unstage_paths(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.unstage(paths)
    }

    /// Discarded work goes into a backup first (`refs/cogit/backup`), so Undo has something
    /// to put back.
    pub(crate) fn local_discard_paths(
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
        let known = handle.submodule_paths();
        let (modules, files): (Vec<String>, Vec<String>) =
            paths.iter().cloned().partition(|path| known.contains(path));
        if !modules.is_empty() {
            handle.submodule_op(git_engine::SubmoduleOp::Reset, &modules)?;
            self.record(repo, format!("Reset {}", named(&modules)), Recovery::None);
        }
        if files.is_empty() {
            return Ok(());
        }
        let paths = &handle.with_unstaged_rename_sources(&files)?;
        if matches!(handle.head()?, git_engine::Head::Unborn { .. }) {
            let kept = handle
                .keep_files(&files)
                .map_err(|err| backup_failed("discarding", &err))?;
            handle.discard(paths)?;
            let left = handle.file_hashes(paths)?;
            self.record(
                repo,
                format!("Discard {}", named(paths)),
                Recovery::Discarded {
                    kept: files.into_iter().zip(kept).collect(),
                    left: paths.iter().cloned().zip(left).collect(),
                },
            );
            return Ok(());
        }
        // Read before the stash, which empties the folder at a new file: only these did it take.
        let taken = handle.stashable(paths)?;
        // A successful stash has already taken the changes out of the working tree, so
        // discarding again would only fail on paths Git no longer knows about.
        let stashed = handle
            .backup_paths(paths, &format!("cogit: discard {}", named(paths)))
            .map_err(|err| backup_failed("discarding", &err))?;
        let Some(oid) = stashed else {
            handle.discard(paths)?;
            self.record(repo, format!("Discard {}", named(paths)), Recovery::None);
            return Ok(());
        };

        // A stash of paths takes their staged side too, and only the unstaged one goes.
        let staged = handle.staged_in_stash(&oid, &taken)?;
        if staged.is_empty() {
            self.record(
                repo,
                format!("Discard {}", named(paths)),
                Recovery::Stash { oid },
            );
            return Ok(());
        }
        let restored = handle.restore_staged_from(&oid, &staged);
        // Recorded whatever came of it: until the restore, the staged side is in the stash only.
        self.record(
            repo,
            format!("Discard {}", named(paths)),
            Recovery::Discard { staged, stash: oid },
        );
        restored
    }

    pub(crate) fn local_add_to_gitignore(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_to_gitignore(paths)
    }

    pub(crate) fn local_stage_mode(
        &self,
        repo: RepoId,
        path: &str,
        executable: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage_mode(path, executable)
    }

    pub(crate) fn local_commit(
        &self,
        repo: RepoId,
        request: &git_engine::CommitRequest,
    ) -> Result<String, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let finishing = handle
            .state()
            .is_ok_and(|state| state.is_interrupted_operation());
        // What an amend replaces: undo puts HEAD back on it softly, the amended changes
        // staged again, so nothing the amend took in is lost and no dialog has to ask first.
        let replaced = if request.amend && !finishing {
            match handle.head()? {
                git_engine::Head::Branch { name, oid } => Some((Some(name), oid)),
                git_engine::Head::Detached { oid } => Some((None, oid)),
                git_engine::Head::Unborn { .. } => None,
            }
        } else {
            None
        };
        let oid = handle.commit(request)?;
        if finishing {
            self.settle_moves(repo);
        }
        if let Some((branch, was)) = replaced {
            self.record(
                repo,
                format!("Amend {}", &was[..was.len().min(8)]),
                crate::Recovery::Reset {
                    branch,
                    oid: was,
                    mode: git_engine::ResetMode::Soft,
                    stash: None,
                    kept: Vec::new(),
                    after: Some(oid.clone()),
                },
            );
        }
        Ok(oid)
    }

    pub(crate) fn local_wants_maintenance(
        &self,
        repo: RepoId,
    ) -> Result<bool, git_engine::GitError> {
        Ok(self.handle(repo)?.wants_maintenance())
    }

    /// After a commit, as a write of its own: gc packs refs and expires reflogs (R-444).
    pub(crate) fn local_maintain_after_commit(
        &self,
        repo: RepoId,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.maintain_after_commit();
        Ok(())
    }
}
