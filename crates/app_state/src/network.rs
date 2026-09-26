//! The commands that talk to a remote. Credentials never appear here: the system `git`
//! asks its own helper, and the token this looks up is only for the hosting API.

use crate::{AppState, RepoId, host_of};
use git_engine::NetworkStop;

/// A network command running as one queue operation; `cancel_network` finds it until
/// this is dropped.
#[derive(Debug)]
pub struct NetworkRun<'a> {
    state: &'a AppState,
    operation: u32,
    stop: NetworkStop,
}

impl NetworkRun<'_> {
    #[must_use]
    pub fn token(&self) -> NetworkStop {
        self.stop.clone()
    }
}

impl Drop for NetworkRun<'_> {
    fn drop(&mut self) {
        self.state.network_runs.lock().remove(&self.operation);
    }
}

impl AppState {
    #[must_use]
    pub fn network_stop(&self, operation: u32) -> NetworkRun<'_> {
        let stop = NetworkStop::default();
        self.network_runs.lock().insert(operation, stop.clone());
        NetworkRun {
            state: self,
            operation,
            stop,
        }
    }

    /// Stops the fetch, pull or push running as `operation`. `false` when there is none:
    /// it has finished, is still waiting in the queue, is not a network command, or was
    /// asked to stop already.
    pub fn cancel_network(&self, operation: u32) -> bool {
        let stop = self.network_runs.lock().get(&operation).cloned();
        let stopped = stop.is_some_and(|stop| stop.stop());
        tracing::info!(operation, stopped, "network cancel requested");
        stopped
    }

    pub fn remotes(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.remotes()
    }

    pub fn fetch(
        &self,
        repo: RepoId,
        remote: &str,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.fetch(remote, |url| self.token_for(url), on_line)
    }

    pub fn pull(
        &self,
        repo: RepoId,
        remote: &str,
        ff_only: bool,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        // Listed before the pull: a submodule the user deinitialised stays that way (#42).
        let known = handle
            .wants_new_submodules()
            .then(|| handle.submodule_paths());
        let before = handle.head()?;
        let result = handle.pull(remote, ff_only, |url| self.token_for(url), on_line);
        self.record_move(
            repo,
            &handle,
            before,
            format!("Pull from {remote}"),
            &result,
        );
        result?;
        if let Some(known) = known {
            handle.init_submodules_added_since(&known)?;
        }
        Ok(())
    }

    pub fn push(
        &self,
        repo: RepoId,
        remote: &str,
        force: bool,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.push(remote, None, force, |url| self.token_for(url), on_line)
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
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.push(
            remote,
            Some(refspec),
            false,
            |url| self.token_for(url),
            on_line,
        )
    }

    /// Only for an HTTP remote: SSH already authenticates through the agent, and handing
    /// a token to an unknown host would leak it.
    fn token_for(&self, url: &str) -> Option<String> {
        if !git_engine::wants_auth(url) {
            return None;
        }
        self.secrets.get(&host_of(url)?)
    }
}
