//! Maintenance, ignore rules, LFS locks, signatures, rerere and range-diff.

use crate::{AppState, RepoId};
use git_engine::GitError;

impl AppState {
    pub(crate) fn local_run_maintenance(
        &self,
        repo: RepoId,
        task: git_engine::MaintenanceTask,
        prune: git_engine::GcPrune,
    ) -> Result<(), GitError> {
        self.handle(repo)?.run_maintenance(task, prune)
    }

    pub(crate) fn local_add_to_exclude(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.add_to_exclude(paths)
    }

    pub(crate) fn local_ignore_rules(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<Vec<git_engine::IgnoreRule>, GitError> {
        self.handle(repo)?.ignore_rules(paths)
    }

    pub(crate) fn local_lfs_locks(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::LfsLock>, GitError> {
        self.handle(repo)?.lfs_locks()
    }

    pub(crate) fn local_lfs_file_states(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<Vec<git_engine::LfsFileState>, GitError> {
        // Read on every refresh of the Files list: without git-lfs it would fail each time.
        self.unjournaled_handle(repo)?.lfs_file_states(paths)
    }

    pub(crate) fn local_commit_signature(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<git_engine::SignatureCheck, GitError> {
        self.handle(repo)?.commit_signature(rev)
    }

    pub(crate) fn local_commit_signatures(
        &self,
        repo: RepoId,
        oids: &[String],
    ) -> Result<Vec<git_engine::SignatureStatus>, GitError> {
        self.handle(repo)?.commit_signatures(oids)
    }

    pub(crate) fn local_unportable_paths(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<String>, GitError> {
        self.handle(repo)?.unportable_paths(rev)
    }

    pub(crate) fn local_rerere_status(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::RerereStatus, GitError> {
        self.handle(repo)?.rerere_status()
    }

    pub(crate) fn local_rerere_forget(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<(), GitError> {
        self.handle(repo)?.rerere_forget(paths)
    }

    pub(crate) fn local_range_diff(
        &self,
        repo: RepoId,
        before: &str,
        after: &str,
    ) -> Result<git_engine::GitOutput, GitError> {
        self.handle(repo)?.range_diff(before, after)
    }
}
