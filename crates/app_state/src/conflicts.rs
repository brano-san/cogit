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

/// Past the solver's limit a conflict is taken whole, in the preview as in the solver.
pub(crate) fn too_large(
    handle: &git_engine::RepoHandle,
    path: &str,
) -> Result<bool, git_engine::GitError> {
    Ok(handle.largest_conflict_side(path)? > crate::MAX_SOLVER_BYTES as u64)
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
        let handle = self.handle(repo)?;
        if too_large(&handle, path)? {
            let stages = handle.conflict_stages(path)?;
            return Ok(git_engine::ConflictText {
                too_large: true,
                missing_ours: stages.ours.is_none(),
                missing_theirs: stages.theirs.is_none(),
                stages,
                ..Default::default()
            });
        }
        Ok(handle.conflict_sides(path)?.to_text())
    }

    pub fn resolve_conflict(
        &self,
        repo: RepoId,
        path: &str,
        side: git_engine::ConflictSide,
        expected: Option<&git_engine::ConflictStages>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let kept = keep_for_undo(&handle, path)?;
        let taken = format!("{side:?}").to_lowercase();
        let description = format!("Take {taken} for {path}");
        let done = handle.resolve_with(path, side, expected);
        self.record_what_was_done(repo, &handle, description, path, kept, done)
    }

    pub fn resolve_conflict_text(
        &self,
        repo: RepoId,
        path: &str,
        text: &str,
        expected: Option<&git_engine::ConflictStages>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let kept = keep_for_undo(&handle, path)?;
        let done = handle.resolve_with_text(path, text, expected);
        self.record_what_was_done(repo, &handle, format!("Resolve {path}"), path, kept, done)
    }

    /// A step after the first change can fail (a smudge filter, a file held open): the
    /// error is returned, but a conflict already settled in the index, or a working file
    /// already rewritten, still gets its entry, or nothing could bring it back.
    fn record_what_was_done(
        &self,
        repo: RepoId,
        handle: &git_engine::RepoHandle,
        description: String,
        path: &str,
        kept: Option<String>,
        done: Result<(), git_engine::GitError>,
    ) -> Result<(), git_engine::GitError> {
        if let Err(err) = &done {
            let settled = handle
                .conflicted_paths()
                .is_ok_and(|paths| !paths.iter().any(|p| p == path));
            let rewritten = handle.keep_worktree_file(path).ok() != Some(kept.clone());
            if !settled && !rewritten {
                return done;
            }
            tracing::warn!(error = ?err, path, "a resolution failed halfway; Undo is kept");
        }
        self.record_resolution(repo, description, path, kept);
        done
    }

    pub(crate) fn record_resolution(
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
