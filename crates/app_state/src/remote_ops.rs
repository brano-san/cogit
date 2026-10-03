//! Remote ▸ Submodule, Subtree and LFS (#45, #46) and Repository ▸ Settings (#42).

use crate::{AppState, RepoId};
use git_engine::{
    GitError, LfsOp, NetworkStop, RepoSetting, RepoSettingChange, SubmoduleOp, SubtreeOp,
};

impl AppState {
    pub fn submodule_op(
        &self,
        repo: RepoId,
        op: SubmoduleOp,
        paths: &[String],
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.handle(repo)?
            .with_stop(stop.clone())
            .submodule_op(op, paths)
    }

    pub fn add_submodule(
        &self,
        repo: RepoId,
        url: &str,
        path: &str,
        branch: Option<&str>,
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.handle(repo)?
            .with_stop(stop.clone())
            .add_submodule(url, path, branch)
    }

    pub fn subtree_op(
        &self,
        repo: RepoId,
        op: &SubtreeOp,
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.handle(repo)?.with_stop(stop.clone()).subtree_op(op)
    }

    pub fn subtree_prefixes(&self, repo: RepoId) -> Result<Vec<String>, GitError> {
        self.handle(repo)?.subtree_prefixes()
    }

    pub fn lfs_op(&self, repo: RepoId, op: &LfsOp, stop: &NetworkStop) -> Result<(), GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.handle(repo)?.with_stop(stop.clone()).lfs_op(op)
    }

    pub fn repo_settings(&self, repo: RepoId) -> Result<Vec<RepoSetting>, GitError> {
        Ok(self.handle(repo)?.repo_settings())
    }

    pub fn write_repo_settings(
        &self,
        repo: RepoId,
        changes: &[RepoSettingChange],
    ) -> Result<(), GitError> {
        self.handle(repo)?.write_repo_settings(changes)
    }
}
