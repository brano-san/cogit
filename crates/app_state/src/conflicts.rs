//! Conflicted paths and their resolution, the working file kept for Undo first.

use crate::{AppState, Recovery, RepoId, backup_failed};

/// The working file before a resolution writes over it (INV-12); without the copy the
/// resolution does not go ahead.
fn keep_for_undo(
    handle: &git_engine::RepoHandle,
    path: &str,
) -> Result<Option<String>, git_engine::GitError> {
    handle
        .keep_worktree_file(path)
        .map_err(|err| backup_failed("resolving", &err))
}

impl AppState {
    pub fn conflicted_paths(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.conflicted_paths()
    }

    pub fn conflict_text(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<git_engine::ConflictText, git_engine::GitError> {
        Ok(self.handle(repo)?.conflict_sides(path)?.to_text())
    }

    pub fn resolve_conflict(
        &self,
        repo: RepoId,
        path: &str,
        side: git_engine::ConflictSide,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let kept = keep_for_undo(&handle, path)?;
        handle.resolve_with(path, side)?;
        let taken = format!("{side:?}").to_lowercase();
        self.record_resolution(repo, format!("Take {taken} for {path}"), path, kept);
        Ok(())
    }

    pub fn resolve_conflict_text(
        &self,
        repo: RepoId,
        path: &str,
        text: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let kept = keep_for_undo(&handle, path)?;
        handle.resolve_with_text(path, text)?;
        self.record_resolution(repo, format!("Resolve {path}"), path, kept);
        Ok(())
    }

    fn record_resolution(
        &self,
        repo: RepoId,
        description: String,
        path: &str,
        kept: Option<String>,
    ) {
        let recovery = Recovery::Resolution {
            path: path.to_owned(),
            kept,
        };
        self.record(repo, description, recovery);
    }
}
