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
        git_engine::read_config(&self.config_target(repo, scope)?)
    }

    pub fn save_config_file(
        &self,
        repo: Option<RepoId>,
        scope: git_engine::ConfigScope,
        text: &str,
        crlf: bool,
    ) -> Result<(), git_engine::GitError> {
        git_engine::save_config(&self.config_target(repo, scope)?, text, crlf)
    }

    pub fn remote_url(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self.handle(repo)?.remote_url(name))
    }

    /// The repository and every submodule below it; 72 ms on a tree of twenty-one.
    pub fn health(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::HealthFinding>, git_engine::GitError> {
        Ok(self.handle(repo)?.health_report())
    }
}
