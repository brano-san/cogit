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

        let iter = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(UntrackedFiles::Files)
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

        let mut status = RepoStatus::default();
        let mut conflicted = Vec::new();
        for item in iter {
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
}
