//! Remote ▸ Submodule, Subtree and LFS (#45, #46) and Repository ▸ Settings (#42).

use crate::{AppState, RepoId};
use git_engine::{GitError, LfsOp, RepoSetting, RepoSettingChange, SubmoduleOp, SubtreeOp};

impl AppState {
    pub fn submodule_op(
        &self,
        repo: RepoId,
        op: SubmoduleOp,
        paths: &[String],
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.submodule_op(op, paths)
    }

    pub fn add_submodule(
        &self,
        repo: RepoId,
        url: &str,
        path: &str,
        branch: Option<&str>,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_submodule(url, path, branch)
    }

    pub fn subtree_op(&self, repo: RepoId, op: &SubtreeOp) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.subtree_op(op)
    }

    pub fn subtree_prefixes(&self, repo: RepoId) -> Result<Vec<String>, GitError> {
        self.handle(repo)?.subtree_prefixes()
    }

    pub fn lfs_op(&self, repo: RepoId, op: &LfsOp) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.lfs_op(op)
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
