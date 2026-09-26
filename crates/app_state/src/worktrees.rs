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
        self.open_with(handle, folder, false, watch, began)
    }

    pub fn add_worktree(
        &self,
        repo: RepoId,
        path: &str,
        branch: &str,
        create: bool,
        base: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?
            .add_worktree_at(path, branch, create, base)
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
        let wanted = path.replace('\\', "/");
        let checkout = handle
            .worktree_heads()?
            .into_iter()
            .find(|entry| entry.path == wanted)
            .map(|entry| entry.branch.unwrap_or(entry.head));
        let stashed = if force {
            handle
                .stash_worktree_changes(path, &format!("cogit: before removing worktree {name}"))?
        } else {
            None
        };
        handle.remove_worktree(path, force)?;
        let recovery = match (stashed, checkout) {
            (Some(stash), Some(checkout)) => Recovery::Worktree {
                path: path.to_owned(),
                checkout,
                stash,
            },
            _ => Recovery::None,
        };
        self.record(repo, format!("Remove worktree {name}"), recovery);
        Ok(())
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

    pub fn worktree_changes(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<Vec<git_engine::FileEntry>, git_engine::GitError> {
        self.handle(repo)?.worktree_changes(path)
    }
}
