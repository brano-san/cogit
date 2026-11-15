//! Stashes: list, save, apply, drop, and one stash's contents (M5).

use crate::{AppState, Recovery, RepoId};

impl AppState {
    pub(crate) fn local_stashes(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::StashEntry>, git_engine::GitError> {
        self.handle(repo)?.stashes()
    }

    /// Only the named paths, leaving everything else in the working tree (T5.3).
    pub(crate) fn local_stash_selection(
        &self,
        repo: RepoId,
        paths: &[String],
        message: &str,
    ) -> Result<(), git_engine::GitError> {
        if paths.is_empty() {
            return Err(git_engine::GitError::InvalidState(
                "no paths given; refusing to stash the whole repository".to_owned(),
            ));
        }
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        handle.stash_paths(paths, message).map(drop)
    }

    pub(crate) fn local_stash_contents(
        &self,
        repo: RepoId,
        index: u32,
    ) -> Result<git_engine::StashContents, git_engine::GitError> {
        self.handle(repo)?.stash_contents(index)
    }

    pub(crate) fn local_stash_push(
        &self,
        repo: RepoId,
        options: &git_engine::StashOptions,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stash_push(options)
    }

    pub(crate) fn local_stash_keeping_worktree(
        &self,
        repo: RepoId,
        message: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.stash_keeping_worktree(message)
    }

    pub(crate) fn local_stash_apply(
        &self,
        repo: RepoId,
        oid: &str,
        pop: bool,
        restore_index: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        if !pop {
            return handle.stash_apply_index(oid, false, restore_index);
        }
        // A pop drops the stash once it applies: undone like Drop, which lists it again and
        // leaves the applied work where it is. Git keeps it when the apply conflicts.
        let entry = handle
            .stashes()?
            .into_iter()
            .find(|entry| entry.oid == oid)
            .ok_or_else(|| git_engine::GitError::InvalidState("the stash is gone".to_owned()))?;
        handle
            .pin_for_undo(oid)
            .map_err(|err| crate::backup_failed("popping a stash", &err))?;
        let applied = handle.stash_apply_index(oid, true, restore_index);
        let dropped = handle
            .stashes()
            .map(|left| left.iter().all(|each| each.oid != oid))
            .unwrap_or(false);
        if dropped {
            self.record(
                repo,
                format!("Pop stash@{{{}}} ({})", entry.index, entry.message),
                Recovery::DroppedStash { entry },
            );
        } else {
            self.unpin(&handle, repo, oid, None);
        }
        applied
    }

    /// Only `paths` of the stash, which stays listed (F-945). Refused before anything is
    /// written when a path has local changes; what each path held is kept for Undo.
    pub(crate) fn local_stash_apply_paths(
        &self,
        repo: RepoId,
        oid: &str,
        paths: &[String],
        restore_index: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let pick = handle.stash_pick(oid, paths)?;
        let kept = handle
            .keep_present(pick.paths())
            .map_err(|err| crate::backup_failed("applying the stash over", &err))?;
        let applied = handle.apply_stash_pick(&pick, restore_index);
        // Recorded whatever came of it: a write that stopped halfway still changed files.
        let recovery = match handle.file_hashes(pick.paths()) {
            Ok(left) => Recovery::StashPaths {
                head: pick.head.clone(),
                kept,
                left: pick.paths().iter().cloned().zip(left).collect(),
            },
            Err(err) => {
                tracing::error!(error = ?err, context = "partial stash apply: cannot read what it left; not undoable");
                Recovery::None
            }
        };
        self.record(
            repo,
            format!(
                "Apply {} from stash@{{{}}}",
                crate::named(pick.paths()),
                pick.index
            ),
            recovery,
        );
        applied
    }

    /// Stash, check out, apply: one call, which the command runs as one operation (R-521,
    /// R-563).
    pub(crate) fn local_switch_with_autostash(
        &self,
        repo: RepoId,
        target: &git_engine::CheckoutTarget,
        message: &str,
        drop_after_clean: bool,
    ) -> Result<git_engine::AutostashOutcome, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let outcome =
            self.handle(repo)?
                .switch_with_autostash(target, message, drop_after_clean)?;
        self.record(
            repo,
            format!("Check out {}", target.described()),
            Recovery::None,
        );
        Ok(outcome)
    }

    pub(crate) fn local_stash_drop(
        &self,
        repo: RepoId,
        oid: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        handle
            .pin_for_undo(oid)
            .map_err(|err| crate::backup_failed("dropping a stash", &err))?;
        let entry = handle.stash_drop(oid).inspect_err(|_| {
            self.unpin(&handle, repo, oid, None);
        })?;
        self.record(
            repo,
            format!("Drop stash@{{{}}} ({})", entry.index, entry.message),
            Recovery::DroppedStash { entry },
        );
        Ok(())
    }
}
