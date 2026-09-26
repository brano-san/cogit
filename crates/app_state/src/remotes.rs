//! A remote's own menu in Branches: Properties, Rename, Delete (#19 of 25.09).

use crate::{AppState, RepoId};
use git_engine::{GitError, RemoteInfo};

impl AppState {
    pub fn remote_info(&self, repo: RepoId, name: &str) -> Result<RemoteInfo, GitError> {
        self.handle(repo)?.remote_info(name)
    }

    pub fn rename_remote(&self, repo: RepoId, from: &str, to: &str) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.rename_remote(from, to)
    }

    pub fn remove_remote(&self, repo: RepoId, name: &str) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.remove_remote(name)
    }

    /// Properties ▸ OK: only what changed is written.
    pub fn set_remote_properties(
        &self,
        repo: RepoId,
        name: &str,
        url: &str,
        background_fetch: bool,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let was = handle.remote_info(name)?;
        if was.url.as_deref() != Some(url.trim()) {
            handle.set_remote_url(name, url)?;
        }
        if was.background_fetch != background_fetch {
            handle.set_background_fetch(name, background_fetch)?;
        }
        Ok(())
    }
}
