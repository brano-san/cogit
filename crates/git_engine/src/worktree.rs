use crate::{FileEntry, FileStatus, GitError, RepoHandle, Result};
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change as WorktreeChange, EntryStatus};
use gix::status::{Item, UntrackedFiles};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeFiles {
    pub staged: Vec<FileEntry>,
    /// Unstaged edits, untracked files and conflicts share one section, as in Git's own
    /// "Changes not staged for commit" plus "Untracked files".
    pub unstaged: Vec<FileEntry>,
}

impl RepoHandle {
    pub fn worktree_files(&self) -> Result<WorktreeFiles> {
        let mut files = WorktreeFiles::default();
        if self.repo.is_bare() {
            return Ok(files);
        }

        let iter = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(UntrackedFiles::Files)
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

        for item in iter {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            match item {
                Item::TreeIndex(change) => files.staged.push(staged_entry(&change)),
                Item::IndexWorktree(WorktreeItem::Modification {
                    rela_path, status, ..
                }) => {
                    if let Some(status) = worktree_status(&status) {
                        files.unstaged.push(entry(rela_path.to_string(), status));
                    }
                }
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry: found, .. }) => {
                    if matches!(found.status, gix::dir::entry::Status::Untracked) {
                        files
                            .unstaged
                            .push(entry(found.rela_path.to_string(), FileStatus::Untracked));
                    }
                }
                Item::IndexWorktree(_) => {}
            }
        }

        files.staged.sort_by(|a, b| a.path.cmp(&b.path));
        files.unstaged.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }
}

fn entry(path: String, status: FileStatus) -> FileEntry {
    FileEntry {
        path,
        old_path: None,
        status,
    }
}

fn staged_entry(change: &gix::diff::index::Change) -> FileEntry {
    use gix::diff::index::Change;
    match change {
        Change::Addition { location, .. } => entry(location.to_string(), FileStatus::Added),
        Change::Deletion { location, .. } => entry(location.to_string(), FileStatus::Deleted),
        Change::Modification { location, .. } => entry(location.to_string(), FileStatus::Modified),
        Change::Rewrite {
            location,
            source_location,
            copy,
            ..
        } => FileEntry {
            path: location.to_string(),
            old_path: Some(source_location.to_string()),
            status: if *copy {
                FileStatus::Copied
            } else {
                FileStatus::Renamed
            },
        },
    }
}

fn worktree_status(status: &EntryStatus<(), gix::submodule::Status>) -> Option<FileStatus> {
    match status {
        EntryStatus::Conflict { .. } => Some(FileStatus::Conflicted),
        EntryStatus::Change(WorktreeChange::Removed) => Some(FileStatus::Deleted),
        EntryStatus::Change(_) => Some(FileStatus::Modified),
        EntryStatus::NeedsUpdate(_) | EntryStatus::IntentToAdd => None,
    }
}
