//! Operations that rewrite history or replay commits, each recorded for Undo (M4, M12).

use crate::{AppState, Recovery, RepoId, backup_failed, named, short};

impl AppState {
    pub fn abort_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.abort_operation()?;
        self.record(
            repo,
            "Abort the operation in progress".to_owned(),
            Recovery::None,
        );
        Ok(())
    }

    pub fn continue_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.continue_operation()
    }

    /// Stashes first when the tree is dirty: restoring a past version must not quietly
    /// overwrite work in progress (doc/modules/M12-commit-surgery.md).
    pub fn rollback_to(
        &self,
        repo: RepoId,
        rev: &str,
        paths: &[String],
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let label = if paths.is_empty() {
            "the working tree".to_owned()
        } else {
            named(paths)
        };
        let stashed = handle
            .stash_paths(paths, &format!("cogit: before rollback of {label}"))
            .map_err(|err| backup_failed("rolling back", &err))?;

        handle.rollback_to(rev, paths)?;
        self.record(
            repo,
            format!("Roll back {label} to {}", short(rev)),
            Recovery::Rollback {
                paths: paths.to_vec(),
                stash: stashed,
            },
        );
        Ok(())
    }

    /// Only the rows on screen: the whole history would be tens of thousands of tree
    /// comparisons for data nobody looks at (doc/modules/M13-commit-overlap.md).
    pub fn overlap_window(
        &self,
        repo: RepoId,
        base: &str,
        window: &[String],
    ) -> Result<Vec<git_engine::OverlapRow>, git_engine::GitError> {
        self.handle(repo)?.overlap_window(base, window)
    }

    pub fn rebase_progress(
        &self,
        repo: RepoId,
    ) -> Result<Option<git_engine::RebaseProgress>, git_engine::GitError> {
        self.handle(repo)?.rebase_progress()
    }

    pub fn rebase_todo(
        &self,
        repo: RepoId,
        base: &str,
    ) -> Result<Vec<git_engine::TodoEntry>, git_engine::GitError> {
        self.handle(repo)?.rebase_todo(base)
    }

    pub fn interactive_rebase(
        &self,
        repo: RepoId,
        base: &str,
        plan: &[git_engine::TodoEntry],
        paused: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        if paused {
            handle.interactive_rebase_paused(base, plan)?;
        } else {
            handle.interactive_rebase(base, plan)?;
        }

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Moved { name, oid },
            _ => Recovery::None,
        };
        self.record(
            repo,
            format!("Interactive rebase onto {}", short(base)),
            recovery,
        );
        Ok(())
    }

    /// The shared branches that already contain this commit; empty means safe to rewrite.
    pub fn protecting_refs(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.protecting_refs(rev)
    }

    pub fn is_published(&self, repo: RepoId, rev: &str) -> Result<bool, git_engine::GitError> {
        self.handle(repo)?.is_published(rev)
    }

    pub fn is_merged_into_head(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<bool, git_engine::GitError> {
        self.handle(repo)?.is_merged_into_head(rev)
    }

    pub fn split_off(
        &self,
        repo: RepoId,
        rev: &str,
        paths: &[String],
        message: &str,
        split_first: bool,
    ) -> Result<(), git_engine::GitError> {
        let handle = self.handle(repo)?;
        let protecting = handle.protecting_refs(rev)?;
        if !protecting.is_empty() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{} is on {} — splitting it would rewrite what others already have",
                short(rev),
                protecting.join(", ")
            )));
        }

        let _quiet = self.quiet(repo);
        let before = handle.head()?;
        handle.split_off(rev, paths, message, split_first)?;

        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Moved { name, oid },
            _ => Recovery::None,
        };
        self.record(
            repo,
            format!("Split {} off {}", named(paths), short(rev)),
            recovery,
        );
        Ok(())
    }

    pub fn merge(
        &self,
        repo: RepoId,
        options: &git_engine::MergeOptions,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        let result = handle.merge(options);
        self.record_move(
            repo,
            &handle,
            before,
            format!("Merge {}", options.source),
            &result,
        );
        result
    }

    pub fn rebase(
        &self,
        repo: RepoId,
        options: &git_engine::RebaseOptions,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        let result = handle.rebase(options);
        self.record_move(
            repo,
            &handle,
            before,
            format!("Rebase onto {}", options.onto),
            &result,
        );
        result
    }

    pub fn skip_operation(&self, repo: RepoId) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.skip_operation()
    }

    pub fn cherry_pick(
        &self,
        repo: RepoId,
        commits: &[String],
    ) -> Result<(), git_engine::GitError> {
        self.replay(repo, commits, true)
    }

    pub fn revert(&self, repo: RepoId, commits: &[String]) -> Result<(), git_engine::GitError> {
        self.replay(repo, commits, false)
    }

    fn replay(
        &self,
        repo: RepoId,
        commits: &[String],
        pick: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        let result = if pick {
            handle.cherry_pick(commits)
        } else {
            handle.revert(commits)
        };
        let verb = if pick { "Cherry-pick" } else { "Revert" };
        let what = format!("{verb} {} commit(s)", commits.len());
        self.record_move(repo, &handle, before, what, &result);
        result
    }

    /// Recorded when the branch moved, and also when the operation stopped on a conflict:
    /// the user finishes that one later, by Continue or by a commit, and Undo must still
    /// know where the branch was before it began (INV-12).
    pub(crate) fn record_move<T>(
        &self,
        repo: RepoId,
        handle: &git_engine::RepoHandle,
        before: git_engine::Head,
        what: String,
        result: &Result<T, git_engine::GitError>,
    ) {
        let stopped = result.is_err()
            && handle
                .state()
                .is_ok_and(|state| state.is_interrupted_operation());
        if result.is_err() && !stopped {
            return;
        }
        let recovery = match before {
            git_engine::Head::Branch { name, oid } => Recovery::Moved { name, oid },
            _ => Recovery::None,
        };
        self.record(repo, what, recovery);
    }
}
