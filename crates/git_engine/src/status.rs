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
    /// `index_lock`: the watcher's index refresh reads only this, and the banner follows it.
    pub index_lock: Option<String>,
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
            if crate::worktree::inert(&item) {
                continue;
            }
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
                    EntryStatus::Change(_) | EntryStatus::IntentToAdd => status.unstaged += 1,
                    EntryStatus::NeedsUpdate(_) => {}
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
        Ok(WorkingState {
            status,
            conflicted,
            index_lock: self.index_lock(),
        })
    }

    /// Any unmerged index entry; the index is read, the working tree is not.
    pub fn has_conflicts(&self) -> Result<bool> {
        if self.repo.is_bare() {
            return Ok(false);
        }
        let index = self.current_index()?;
        Ok(index
            .entries()
            .iter()
            .any(|entry| entry.flags.stage() != gix::index::entry::Stage::Unconflicted))
    }

    /// `!status().is_clean()`, stopping at the first change instead of counting them all.
    pub fn has_changes(&self) -> Result<bool> {
        if self.repo.is_bare() {
            return Ok(false);
        }
        for item in self.status_items()? {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            if crate::worktree::inert(&item) {
                continue;
            }
            let counts = match item {
                Item::TreeIndex(_) => true,
                Item::IndexWorktree(WorktreeItem::Modification { status: entry, .. }) => {
                    matches!(
                        entry,
                        EntryStatus::Conflict { .. }
                            | EntryStatus::Change(_)
                            | EntryStatus::IntentToAdd
                    )
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

    /// Changed paths (staged or not, each once) and untracked entries, folders collapsed as
    /// in Files. A full walk: unlike `has_changes` it cannot stop at the first change.
    pub fn change_counts(&self) -> Result<(u32, u32)> {
        // Not `is_bare()`: a linked worktree of a bare repository reads `core.bare = true`.
        if self.repo.workdir().is_none() {
            return Ok((0, 0));
        }
        let mut changed = std::collections::HashSet::new();
        let mut untracked = 0;
        for item in self.status_items()? {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            if crate::worktree::inert(&item) {
                continue;
            }
            match &item {
                Item::IndexWorktree(WorktreeItem::Modification {
                    status: EntryStatus::NeedsUpdate(_),
                    ..
                }) => {}
                Item::TreeIndex(_) | Item::IndexWorktree(WorktreeItem::Modification { .. }) => {
                    changed.insert(item.location().to_owned());
                }
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. })
                    if matches!(entry.status, gix::dir::entry::Status::Untracked) =>
                {
                    untracked += 1;
                }
                Item::IndexWorktree(_) => {}
            }
        }
        Ok((u32::try_from(changed.len()).unwrap_or(u32::MAX), untracked))
    }

    /// Collapsed, like the Files list: a folder of new files is one entry in both, so the
    /// header counts and the list agree and one walk can serve both (R-316).
    fn status_items(&self) -> Result<gix::status::Iter> {
        self.status_platform()?
            .untracked_files(UntrackedFiles::Files)
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))
    }
}
