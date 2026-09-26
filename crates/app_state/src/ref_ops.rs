//! What the graph and Branches context menus run beyond the toolbar's operations.

use crate::{AppState, Recovery, RepoId, short};
use git_engine::{GitError, ResetMode};

impl AppState {
    /// A hard reset stashes the tracked changes it would destroy first, so Undo can bring
    /// them back; the commits it leaves behind stay reachable as lost commits.
    pub fn reset_to(&self, repo: RepoId, rev: &str, mode: ResetMode) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let (branch, before) = match handle.head()? {
            git_engine::Head::Branch { name, oid } => (Some(name), oid),
            git_engine::Head::Detached { oid } => (None, oid),
            git_engine::Head::Unborn { .. } => {
                return Err(GitError::InvalidState(
                    "there is no commit to reset yet".to_owned(),
                ));
            }
        };
        let from = format!(
            "{} from {}",
            branch.as_deref().unwrap_or("HEAD"),
            short(&before)
        );

        let status = handle.status()?;
        let stashed = if mode == ResetMode::Hard && (status.staged > 0 || status.unstaged > 0) {
            handle
                .backup_before_reset(&format!("cogit: before hard reset to {}", short(rev)))
                .map_err(|err| crate::backup_failed("resetting", &err))?
        } else {
            None
        };

        if let Err(err) = handle.reset(rev, mode) {
            if let Some(oid) = &stashed {
                // The reset never happened, so the work goes back where it was.
                match handle.stash_apply(oid) {
                    Ok(()) => handle.forget_backup(oid),
                    Err(restore) => {
                        tracing::error!(error = ?restore, context = "could not restore the pre-reset backup");
                    }
                }
            }
            return Err(err);
        }
        // A reset to where HEAD already was, with nothing stashed, has nothing to undo.
        let moved = match handle.head()? {
            git_engine::Head::Branch { oid, .. } | git_engine::Head::Detached { oid } => {
                oid != before
            }
            git_engine::Head::Unborn { .. } => true,
        };
        let recovery = if moved || stashed.is_some() {
            Recovery::Reset {
                branch,
                oid: before,
                mode,
                stash: stashed,
            }
        } else {
            Recovery::None
        };
        self.record(
            repo,
            format!("Reset {from} to {} ({})", short(rev), mode_name(mode)),
            recovery,
        );
        Ok(())
    }

    pub fn is_ancestor(
        &self,
        repo: RepoId,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, GitError> {
        self.handle(repo)?.is_ancestor(ancestor, descendant)
    }

    pub fn compare_files(
        &self,
        repo: RepoId,
        from: &str,
        to: &str,
    ) -> Result<Vec<git_engine::FileEntry>, GitError> {
        self.handle(repo)?.files_between(from, to)
    }

    pub fn tag_name_problem(&self, repo: RepoId, name: &str) -> Result<Option<String>, GitError> {
        self.handle(repo)?.tag_name_problem(name)
    }

    pub fn tag_message(&self, repo: RepoId, name: &str) -> Result<Option<String>, GitError> {
        self.handle(repo)?.tag_message(name)
    }

    pub fn rename_tag(&self, repo: RepoId, from: &str, to: &str) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.rename_tag(from, to)?;
        self.record(repo, format!("Rename tag {from} to {to}"), Recovery::None);
        Ok(())
    }

    pub fn rename_stash(&self, repo: RepoId, index: u32, message: &str) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.rename_stash(index, message)
    }

    pub fn edit_author(
        &self,
        repo: RepoId,
        rev: &str,
        name: &str,
        email: &str,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let before = handle.head()?;
        handle.edit_author(rev, name, email)?;
        let (was, recovery) = match before {
            git_engine::Head::Branch { name, oid } => (
                format!(" ({name} was at {})", short(&oid)),
                Recovery::Moved { name, oid },
            ),
            git_engine::Head::Detached { oid } => {
                (format!(" (HEAD was at {})", short(&oid)), Recovery::None)
            }
            git_engine::Head::Unborn { .. } => (String::new(), Recovery::None),
        };
        self.record(
            repo,
            format!("Edit the author of {}{was}", short(rev)),
            recovery,
        );
        Ok(())
    }
}

fn mode_name(mode: ResetMode) -> &'static str {
    match mode {
        ResetMode::Soft => "soft",
        ResetMode::Mixed => "mixed",
        ResetMode::Hard => "hard",
        ResetMode::Keep => "keep",
        ResetMode::Merge => "merge",
    }
}
