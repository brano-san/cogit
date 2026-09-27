//! Maintenance, ignore rules, LFS locks, signatures, rerere and range-diff.

use crate::{AppState, RepoId};
use git_engine::GitError;

impl AppState {
    pub fn run_maintenance(
        &self,
        repo: RepoId,
        task: git_engine::MaintenanceTask,
    ) -> Result<(), GitError> {
        self.handle(repo)?.run_maintenance(task)
    }

    pub fn add_to_exclude(&self, repo: RepoId, paths: &[String]) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_to_exclude(paths)
    }

    pub fn ignore_rules(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<Vec<git_engine::IgnoreRule>, GitError> {
        self.handle(repo)?.ignore_rules(paths)
    }

    pub fn lfs_locks(&self, repo: RepoId) -> Result<Vec<git_engine::LfsLock>, GitError> {
        self.handle(repo)?.lfs_locks()
    }

    pub fn lfs_file_states(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<Vec<git_engine::LfsFileState>, GitError> {
        self.handle(repo)?.lfs_file_states(paths)
    }

    pub fn commit_signature(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<git_engine::SignatureCheck, GitError> {
        self.handle(repo)?.commit_signature(rev)
    }

    pub fn unportable_paths(&self, repo: RepoId, rev: &str) -> Result<Vec<String>, GitError> {
        self.handle(repo)?.unportable_paths(rev)
    }

    pub fn rerere_status(&self, repo: RepoId) -> Result<git_engine::RerereStatus, GitError> {
        self.handle(repo)?.rerere_status()
    }

    pub fn rerere_forget(&self, repo: RepoId, paths: &[String]) -> Result<(), GitError> {
        self.handle(repo)?.rerere_forget(paths)
    }

    pub fn range_diff(
        &self,
        repo: RepoId,
        before: &str,
        after: &str,
    ) -> Result<git_engine::GitOutput, GitError> {
        self.handle(repo)?.range_diff(before, after)
    }
}
