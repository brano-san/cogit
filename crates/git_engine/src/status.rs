//! Working tree status.

use crate::{GitError, RepoHandle, Result};
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::EntryStatus;
use gix::status::{Item, UntrackedFiles};
use serde::Serialize;

/// Counts behind the `Working Tree / Index` row of the graph.
///
/// A file changed both before and after `git add` is counted in `staged` **and**
/// `unstaged`: Git tracks the index and the working tree separately, and collapsing
/// them would hide half of what is going on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

impl RepoStatus {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.staged == 0 && self.unstaged == 0 && self.untracked == 0 && self.conflicted == 0
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.staged + self.unstaged + self.untracked + self.conflicted
    }
}

impl RepoHandle {
    /// Compares HEAD, the index and the working tree.
    ///
    /// # Errors
    /// Returns an error if the index or the working tree cannot be read.
    pub fn status(&self) -> Result<RepoStatus> {
        // Nothing to compare against without a working tree, and erroring here would
        // break the graph panel for a valid repository.
        if self.repo.is_bare() {
            return Ok(RepoStatus::default());
        }

        let iter = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(UntrackedFiles::Files)
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

        let mut status = RepoStatus::default();
        for item in iter {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            match item {
                Item::TreeIndex(_) => status.staged += 1,
                Item::IndexWorktree(WorktreeItem::Modification { status: entry, .. }) => {
                    match entry {
                        EntryStatus::Conflict { .. } => status.conflicted += 1,
                        EntryStatus::Change(_) => status.unstaged += 1,
                        // `NeedsUpdate` is a stat-cache hint, not a change the user made.
                        EntryStatus::NeedsUpdate(_) | EntryStatus::IntentToAdd => {}
                    }
                }
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
                    if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                        status.untracked += 1;
                    }
                }
                Item::IndexWorktree(_) => {}
            }
        }
        Ok(status)
    }
}
