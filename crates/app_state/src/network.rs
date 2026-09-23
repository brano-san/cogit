//! The commands that talk to a remote. Credentials never appear here: the system `git`
//! asks its own helper, and the token this looks up is only for the hosting API.

use crate::{AppState, RepoId, host_of};

impl AppState {
    pub fn remotes(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.remotes()
    }

    pub fn fetch(
        &self,
        repo: RepoId,
        remote: &str,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        handle.fetch(remote, token.as_deref(), on_line)
    }

    pub fn pull(
        &self,
        repo: RepoId,
        remote: &str,
        ff_only: bool,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        handle.pull(remote, ff_only, token.as_deref(), on_line)
    }

    pub fn push(
        &self,
        repo: RepoId,
        remote: &str,
        force: bool,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        handle.push(remote, None, force, token.as_deref(), on_line)
    }

    /// Pull ▸ Delete merged branches after Pull (#26). Each deletion is journalled, so
    /// Undo brings a branch back; one that refuses is logged and the rest still go.
    pub fn delete_merged_branches(
        &self,
        repo: RepoId,
    ) -> Result<Vec<String>, git_engine::GitError> {
        let names = self.handle(repo)?.merged_gone_branches()?;
        let mut deleted = Vec::with_capacity(names.len());
        for name in names {
            match self.delete_branch(repo, &name, false) {
                Ok(()) => deleted.push(name),
                Err(err) => {
                    tracing::error!(error = ?err, branch = %name, context = "failed to delete a merged branch");
                }
            }
        }
        Ok(deleted)
    }

    /// One refspec to one remote: Push To, Push Up To and pushing a ref that is not HEAD.
    pub fn push_to(
        &self,
        repo: RepoId,
        remote: &str,
        refspec: &str,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let token = self.token_for(&handle, remote);
        handle.push(remote, Some(refspec), false, token.as_deref(), on_line)
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
}
