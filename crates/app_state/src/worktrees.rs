//! Linked worktrees: listing them, adding, removing and pruning.

use crate::{AppState, RepoId};

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
}
