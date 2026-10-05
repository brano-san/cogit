use crate::{ConflictKind, FileEntry, FileStatus, GitError, RepoHandle, Result};
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change as WorktreeChange, EntryStatus};
use gix::status::{Item, UntrackedFiles};
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

/// The four Files-panel toggles that cost extra reading. The rest — untracked, rename
/// sources, directories, index and worktree side by side — are filters over what is already
/// in the list, so the panel applies them itself (doc/modules/M6-staging.md, T6.9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct WorktreeView {
    pub unchanged: bool,
    pub ignored: bool,
    pub assume_unchanged: bool,
    pub skipped: bool,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeFiles {
    pub staged: Vec<FileEntry>,
    /// Unstaged edits, untracked files and conflicts share one section, as in Git's own
    /// "Changes not staged for commit" plus "Untracked files".
    pub unstaged: Vec<FileEntry>,
    /// The counters and conflicted paths of the same walk, so one read feeds the list and
    /// the header (R-316).
    pub state: crate::WorkingState,
}

impl RepoHandle {
    #[must_use]
    pub fn sparse_checkout(&self) -> bool {
        self.repo
            .config_snapshot()
            .boolean("core.sparseCheckout")
            .unwrap_or(false)
    }

    pub fn worktree_files(&self) -> Result<WorktreeFiles> {
        self.worktree_files_with(WorktreeView::default())
    }

    pub fn worktree_files_with(&self, view: WorktreeView) -> Result<WorktreeFiles> {
        self.files_with(view, false)
    }

    /// `skip_submodules` leaves a submodule's own status unread: the caller asks for it
    /// apart, in parallel (the Remove Worktree scan, R-675).
    pub(crate) fn files_with(
        &self,
        view: WorktreeView,
        skip_submodules: bool,
    ) -> Result<WorktreeFiles> {
        let mut files = WorktreeFiles::default();
        if self.repo.is_bare() {
            return Ok(files);
        }

        let mut platform = self
            .status_platform()?
            .untracked_files(UntrackedFiles::Files);
        if skip_submodules {
            platform = platform.index_worktree_submodules(gix::status::Submodule::Given {
                ignore: gix::submodule::config::Ignore::All,
                check_dirty: false,
            });
        }
        if view.ignored {
            platform = platform.index_worktree_options_mut(|options| {
                if let Some(walk) = options.dirwalk_options.as_mut() {
                    walk.set_emit_ignored(Some(gix::dir::walk::EmissionMode::Matching));
                }
            });
        }

        let iter = platform
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

        for item in iter {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            if inert(&item) {
                continue;
            }
            match item {
                Item::TreeIndex(change) => files.staged.push(staged_entry(&change)),
                Item::IndexWorktree(WorktreeItem::Modification {
                    rela_path,
                    status,
                    entry: found,
                    ..
                }) => {
                    if let Some(file_status) = worktree_status(&status) {
                        let mut row =
                            entry_of(rela_path.to_string(), file_status, mode_of(found.mode));
                        row.mode_change = executable_bit_change(&status, found.mode);
                        row.submodule = submodule_change(&status);
                        row.conflict = conflict_kind(&status);
                        files.unstaged.push(row);
                    }
                }
                Item::IndexWorktree(WorktreeItem::DirectoryContents { entry: found, .. }) => {
                    let status = match found.status {
                        gix::dir::entry::Status::Untracked => FileStatus::Untracked,
                        gix::dir::entry::Status::Ignored(_) if view.ignored => FileStatus::Ignored,
                        _ => continue,
                    };
                    files
                        .unstaged
                        .push(entry(found.rela_path.to_string(), status));
                }
                Item::IndexWorktree(_) => {}
            }
        }

        // The dirwalk can also report the file a conflict left in the tree as untracked
        // (seen on Linux when one side deleted it); git lists only the conflict.
        let conflicted: BTreeSet<String> = files
            .unstaged
            .iter()
            .filter(|file| file.status == FileStatus::Conflicted)
            .map(|file| file.path.clone())
            .collect();
        files.unstaged.retain(|file| {
            file.status != FileStatus::Untracked || !conflicted.contains(&file.path)
        });

        files.state = self.state_of(&files);

        if view.unchanged || view.assume_unchanged || view.skipped {
            self.add_index_entries(&mut files, view)?;
        }

        files.staged.sort_by(|a, b| a.path.cmp(&b.path));
        files.unstaged.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// What `working_state` counts, from rows already read; ignored rows are not changes.
    fn state_of(&self, files: &WorktreeFiles) -> crate::WorkingState {
        let mut state = crate::WorkingState {
            index_lock: self.index_lock(),
            ..Default::default()
        };
        state.status.staged = u32::try_from(files.staged.len()).unwrap_or(u32::MAX);
        for file in &files.unstaged {
            match file.status {
                FileStatus::Conflicted => {
                    state.status.conflicted += 1;
                    state.conflicted.push(file.path.clone());
                }
                FileStatus::Untracked => state.status.untracked += 1,
                FileStatus::Ignored => {}
                _ => state.status.unstaged += 1,
            }
        }
        state.conflicted.sort();
        state.conflicted.dedup();
        state
    }

    /// The quiet side of the index: files status never mentions because nothing happened to
    /// them, plus the two flags that deliberately silence a file.
    fn add_index_entries(&self, files: &mut WorktreeFiles, view: WorktreeView) -> Result<()> {
        let index = self.current_index()?;
        let sparse = self.sparse_checkout();

        let mentioned: BTreeSet<String> = files
            .staged
            .iter()
            .chain(files.unstaged.iter())
            .map(|file| file.path.clone())
            .collect();

        for item in index.entries() {
            use gix::index::entry::Flags;
            let path = item.path(&index).to_string();
            if mentioned.contains(&path) {
                continue;
            }

            let status = if item.flags.contains(Flags::SKIP_WORKTREE) {
                let hidden = sparse && std::fs::symlink_metadata(self.root().join(&path)).is_err();
                view.skipped.then_some(if hidden {
                    FileStatus::Sparse
                } else {
                    FileStatus::Skipped
                })
            } else if item.flags.contains(Flags::ASSUME_VALID) {
                view.assume_unchanged.then_some(FileStatus::AssumeUnchanged)
            } else {
                view.unchanged.then_some(FileStatus::Unchanged)
            };

            if let Some(status) = status {
                files.unstaged.push(entry(path, status));
            }
        }
        Ok(())
    }
}

fn entry(path: String, status: FileStatus) -> FileEntry {
    entry_of(path, status, crate::FileMode::Plain)
}

fn entry_of(path: String, status: FileStatus, mode: crate::FileMode) -> FileEntry {
    FileEntry {
        path,
        old_path: None,
        status,
        mode,
        mode_change: None,
        similarity: None,
        submodule: None,
        conflict: None,
    }
}

/// The index keeps a raw mode: a gitlink is 0o160000, a symlink 0o120000 (R-143, R-180).
fn mode_of(mode: gix::index::entry::Mode) -> crate::FileMode {
    use gix::index::entry::Mode;
    if mode == Mode::COMMIT {
        crate::FileMode::Submodule
    } else if mode == Mode::SYMLINK {
        crate::FileMode::Symlink
    } else if mode == Mode::FILE_EXECUTABLE {
        crate::FileMode::Executable
    } else {
        crate::FileMode::Plain
    }
}

fn staged_entry(change: &gix::diff::index::Change) -> FileEntry {
    use gix::diff::index::Change;
    match change {
        Change::Addition {
            location,
            entry_mode,
            ..
        } => entry_of(
            location.to_string(),
            FileStatus::Added,
            mode_of(*entry_mode),
        ),
        Change::Deletion {
            location,
            entry_mode,
            ..
        } => entry_of(
            location.to_string(),
            FileStatus::Deleted,
            mode_of(*entry_mode),
        ),
        Change::Modification {
            location,
            entry_mode,
            previous_entry_mode,
            ..
        } => FileEntry {
            mode_change: (entry_mode != previous_entry_mode).then(|| mode_of(*entry_mode)),
            ..entry_of(
                location.to_string(),
                FileStatus::Modified,
                mode_of(*entry_mode),
            )
        },
        Change::Rewrite {
            location,
            source_location,
            copy,
            entry_mode,
            ..
        } => FileEntry {
            path: location.to_string(),
            old_path: Some(source_location.to_string()),
            status: if *copy {
                FileStatus::Copied
            } else {
                FileStatus::Renamed
            },
            mode: mode_of(*entry_mode),
            mode_change: None,
            similarity: None,
            submodule: None,
            conflict: None,
        },
    }
}

/// The mode the file on disk has when only its executable bit differs from the index. A
/// type change (file to symlink) gets none: the `+x` button would stage a plain mode for it.
fn executable_bit_change(
    status: &EntryStatus<(), gix::submodule::Status>,
    indexed: gix::index::entry::Mode,
) -> Option<crate::FileMode> {
    match status {
        EntryStatus::Change(WorktreeChange::Modification {
            executable_bit_changed: true,
            ..
        }) => Some(if indexed == gix::index::entry::Mode::FILE_EXECUTABLE {
            crate::FileMode::Plain
        } else {
            crate::FileMode::Executable
        }),
        _ => None,
    }
}

fn worktree_status(status: &EntryStatus<(), gix::submodule::Status>) -> Option<FileStatus> {
    match status {
        EntryStatus::Conflict { .. } => Some(FileStatus::Conflicted),
        EntryStatus::Change(WorktreeChange::Removed) => Some(FileStatus::Deleted),
        EntryStatus::Change(_) => Some(FileStatus::Modified),
        // `git add -N`: git lists it as a new file not staged for commit.
        EntryStatus::IntentToAdd => Some(FileStatus::Added),
        EntryStatus::NeedsUpdate(_) => None,
    }
}

fn conflict_kind(status: &EntryStatus<(), gix::submodule::Status>) -> Option<ConflictKind> {
    use gix::status::plumbing::index_as_worktree::Conflict as C;
    let EntryStatus::Conflict { summary, .. } = status else {
        return None;
    };
    Some(match summary {
        C::BothModified => ConflictKind::BothModified,
        C::BothAdded => ConflictKind::BothAdded,
        C::BothDeleted => ConflictKind::BothDeleted,
        C::DeletedByThem => ConflictKind::DeletedByThem,
        C::DeletedByUs => ConflictKind::DeletedByUs,
        C::AddedByUs => ConflictKind::AddedByUs,
        C::AddedByThem => ConflictKind::AddedByThem,
    })
}

/// Nothing Files could do with it: a submodule whose recorded commit did not move (only
/// edits or new files inside, which belong to the submodule's own repository) and a
/// repository nested without a gitlink. SmartGit lists neither; the submodule tree and the
/// Repositories list show their state where they can be opened (doc/12-risks.md).
pub(crate) fn inert(item: &Item) -> bool {
    match item {
        Item::IndexWorktree(WorktreeItem::Modification {
            status: EntryStatus::Change(WorktreeChange::SubmoduleModification(inner)),
            ..
        }) => inner.checked_out_head_id == inner.index_id,
        Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
            entry.disk_kind == Some(gix::dir::entry::Kind::Repository)
        }
        _ => false,
    }
}

fn submodule_change(
    status: &EntryStatus<(), gix::submodule::Status>,
) -> Option<crate::SubmoduleChange> {
    let EntryStatus::Change(WorktreeChange::SubmoduleModification(inner)) = status else {
        return None;
    };
    let mut change = crate::SubmoduleChange {
        new_commits: inner.checked_out_head_id != inner.index_id,
        ..Default::default()
    };
    for item in inner.changes.iter().flatten() {
        match item {
            Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
                change.untracked |= matches!(entry.status, gix::dir::entry::Status::Untracked);
            }
            _ => change.modified = true,
        }
    }
    Some(change)
}
