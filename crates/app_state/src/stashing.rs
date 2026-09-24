//! Stashes: list, save, apply, drop, and one stash's contents (M5).

use crate::{AppState, Recovery, RepoId};

impl AppState {
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
        let _quiet = self.quiet(repo);
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
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stash_push(options)
    }

    pub fn stash_keeping_worktree(
        &self,
        repo: RepoId,
        message: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stash_keeping_worktree(message)
    }

    pub fn stash_apply(
        &self,
        repo: RepoId,
        index: u32,
        pop: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stash_apply_index(index, pop)
    }

    pub fn stash_drop(&self, repo: RepoId, index: u32) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let oid = self.handle(repo)?.stash_drop(index)?;
        self.record(
            repo,
            format!("Drop stash@{{{index}}}"),
            Recovery::Stash { oid },
        );
        Ok(())
    }
}
