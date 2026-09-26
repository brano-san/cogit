//! The working tree and the index: status, staging, discarding, the commit (M6).

use crate::{AppState, Recovery, RepoId, backup_failed, named};

impl AppState {
    pub fn worktree_files(
        &self,
        repo: RepoId,
        view: git_engine::WorktreeView,
    ) -> Result<git_engine::WorktreeFiles, git_engine::GitError> {
        self.handle(repo)?.worktree_files_with(view)
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

    /// Discarded work goes into a backup first (`refs/cogit/backup`), so Undo has something
    /// to put back.
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

    pub fn stage_mode(
        &self,
        repo: RepoId,
        path: &str,
        executable: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stage_mode(path, executable)
    }

    pub fn commit(
        &self,
        repo: RepoId,
        request: &git_engine::CommitRequest,
    ) -> Result<String, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.commit(request)
    }

    pub fn wants_maintenance(&self, repo: RepoId) -> Result<bool, git_engine::GitError> {
        Ok(self.handle(repo)?.wants_maintenance())
    }

    /// After a commit, as a write of its own: gc packs refs and expires reflogs (R-444).
    pub fn maintain_after_commit(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.maintain_after_commit();
        Ok(())
    }
}
