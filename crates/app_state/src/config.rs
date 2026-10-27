//! Git config files, remote URLs and the health report.

use crate::{AppState, RepoId};
use std::path::PathBuf;

impl AppState {
    /// Resolved here on every call: the frontend names a scope, never a path to write.
    fn config_target(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
    ) -> Result<PathBuf, git_engine::GitError> {
        match scope {
            git_engine::ConfigScope::User => Ok(git_engine::user_config_path()),
            git_engine::ConfigScope::Repository => {
                let repo = repo.ok_or_else(|| {
                    git_engine::GitError::InvalidState("no repository is open".to_owned())
                })?;
                self.handle(repo)?.config_path()
            }
        }
    }

    pub fn config_file(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
    ) -> Result<git_engine::ConfigFile, git_engine::GitError> {
        if let Some(remote) = repo.and_then(|repo| self.remote(repo)) {
            let request = crate::backend::Request::ConfigFile {
                repo: remote.id(),
                scope,
            };
            return remote.ask(&request, None);
        }
        git_engine::read_config(&self.config_target(repo, scope)?)
    }

    pub fn save_config_file(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
        text: &str,
        crlf: bool,
    ) -> Result<(), git_engine::GitError> {
        if let Some(remote) = repo.and_then(|repo| self.remote(repo)) {
            let request = crate::backend::Request::SaveConfigFile {
                repo: remote.id(),
                scope,
                text: text.to_owned(),
                crlf,
            };
            return remote.ask(&request, None);
        }
        git_engine::save_config(&self.config_target(repo, scope)?, text, crlf)
    }

    pub(crate) fn local_remote_url(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self.handle(repo)?.remote_url(name))
    }

    /// The repository and every submodule below it; 72 ms on a tree of twenty-one.
    pub(crate) fn local_health(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::HealthFinding>, git_engine::GitError> {
        Ok(self.handle(repo)?.health_report())
    }

    pub(crate) fn local_trust_directory(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        self.handle(repo)?.trust_directory()
    }
}
