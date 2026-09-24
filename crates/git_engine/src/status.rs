use crate::{GitError, RepoHandle, Result};
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::EntryStatus;
use gix::status::{Item, UntrackedFiles};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

/// The counters and the conflicted paths, from one read of the status (R-316).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkingState {
    pub status: RepoStatus,
    /// Sorted, each path once — what `conflicted_paths` lists.
    pub conflicted: Vec<String>,
}

impl RepoStatus {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.staged == 0 && self.unstaged == 0 && self.untracked == 0 && self.conflicted == 0
    }
}

impl RepoHandle {
    pub fn status(&self) -> Result<RepoStatus> {
        Ok(self.working_state()?.status)
    }

    pub fn working_state(&self) -> Result<WorkingState> {
        if self.repo.is_bare() {
            return Ok(WorkingState::default());
        }

        let mut status = RepoStatus::default();
        let mut conflicted = Vec::new();
        for item in self.status_items()? {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            match item {
                Item::TreeIndex(_) => status.staged += 1,
                Item::IndexWorktree(WorktreeItem::Modification {
                    status: entry,
                    rela_path,
                    ..
                }) => match entry {
                    EntryStatus::Conflict { .. } => {
                        status.conflicted += 1;
                        conflicted.push(rela_path.to_string());
                    }
                    EntryStatus::Change(_) => status.unstaged += 1,
                    EntryStatus::NeedsUpdate(_) | EntryStatus::IntentToAdd => {}
                },
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
                    if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                        status.untracked += 1;
                    }
                }
                Item::IndexWorktree(_) => {}
            }
        }
        conflicted.sort();
        conflicted.dedup();
        Ok(WorkingState { status, conflicted })
    }

    /// `!status().is_clean()`, stopping at the first change instead of counting them all.
    pub fn has_changes(&self) -> Result<bool> {
        if self.repo.is_bare() {
            return Ok(false);
        }
        for item in self.status_items()? {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            let counts = match item {
                Item::TreeIndex(_) => true,
                Item::IndexWorktree(WorktreeItem::Modification { status: entry, .. }) => {
                    matches!(entry, EntryStatus::Conflict { .. } | EntryStatus::Change(_))
                }
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
                    matches!(entry.status, gix::dir::entry::Status::Untracked)
                }
                Item::IndexWorktree(_) => false,
            };
            if counts {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn status_items(&self) -> Result<gix::status::Iter> {
        self.repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(UntrackedFiles::Files)
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))
    }
}
