use crate::{FileEntry, FileStatus, GitError, RepoHandle, Result};
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
}

impl RepoHandle {
    pub fn worktree_files(&self) -> Result<WorktreeFiles> {
        self.worktree_files_with(WorktreeView::default())
    }

    pub fn worktree_files_with(&self, view: WorktreeView) -> Result<WorktreeFiles> {
        let mut files = WorktreeFiles::default();
        if self.repo.is_bare() {
            return Ok(files);
        }

        let mut platform = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(UntrackedFiles::Collapsed);
        if view.ignored {
            platform = platform.index_worktree_options_mut(|options| {
                if let Some(walk) = options.dirwalk_options.as_mut() {
                    walk.set_emit_ignored(Some(gix::dir::walk::EmissionMode::CollapseDirectory));
                }
            });
        }

        let iter = platform
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
                    let status = match found.status {
                        gix::dir::entry::Status::Untracked => FileStatus::Untracked,
                        gix::dir::entry::Status::Ignored(_) if view.ignored => FileStatus::Ignored,
                        _ => continue,
                    };
                    // A collapsed directory arrives without its trailing slash; the
                    // UI has to tell "generated/" from a file called "generated".
                    let mut path = found.rela_path.to_string();
                    if found.disk_kind == Some(gix::dir::entry::Kind::Directory) {
                        path.push('/');
                    }
                    files.unstaged.push(entry(path, status));
                }
                Item::IndexWorktree(_) => {}
            }
        }

        if view.unchanged || view.assume_unchanged || view.skipped {
            self.add_index_entries(&mut files, view)?;
        }

        files.staged.sort_by(|a, b| a.path.cmp(&b.path));
        files.unstaged.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// The quiet side of the index: files status never mentions because nothing happened to
    /// them, plus the two flags that deliberately silence a file.
    fn add_index_entries(&self, files: &mut WorktreeFiles, view: WorktreeView) -> Result<()> {
        let index = self
            .repo
            .index()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;

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
                view.skipped.then_some(FileStatus::Skipped)
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
    }
}

fn staged_entry(change: &gix::diff::index::Change) -> FileEntry {
    use gix::diff::index::Change;
    match change {
        Change::Addition { location, .. } => entry(location.to_string(), FileStatus::Added),
        Change::Deletion { location, .. } => entry(location.to_string(), FileStatus::Deleted),
        Change::Modification {
            location,
            entry_mode,
            ..
        } => entry_of(
            location.to_string(),
            FileStatus::Modified,
            // The index reports a raw mode; a gitlink is 0o160000, and telling one apart
            // is the whole point of showing an icon at all (R-143).
            if *entry_mode == gix::index::entry::Mode::COMMIT {
                crate::FileMode::Submodule
            } else if *entry_mode == gix::index::entry::Mode::SYMLINK {
                crate::FileMode::Symlink
            } else {
                crate::FileMode::Plain
            },
        ),
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
            mode: crate::FileMode::Plain,
            mode_change: None,
            similarity: None,
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

impl RepoHandle {
    /// Appends to `.gitignore`, skipping patterns it already contains.
    pub fn add_to_gitignore(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no paths to ignore".to_owned()));
        }

        let file = self.root().join(".gitignore");
        let existing = std::fs::read_to_string(&file).unwrap_or_default();
        let known: std::collections::HashSet<&str> = existing.lines().map(str::trim).collect();

        let mut added = String::new();
        for path in paths {
            let pattern = path.trim();
            if pattern.is_empty() || known.contains(pattern) {
                continue;
            }
            added.push_str(pattern);
            added.push('\n');
        }
        if added.is_empty() {
            return Ok(());
        }

        let mut text = existing;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&added);
        std::fs::write(&file, text)?;
        Ok(())
    }

    /// Only untracked paths: deleting a tracked one is `discard`, which keeps a stash.
    pub fn delete_untracked(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no paths to delete".to_owned()));
        }

        let untracked: std::collections::HashSet<String> = self
            .worktree_files()?
            .unstaged
            .into_iter()
            .filter(|entry| entry.status == FileStatus::Untracked)
            .map(|entry| entry.path)
            .collect();

        for path in paths {
            if !untracked.contains(path) {
                return Err(GitError::InvalidState(format!(
                    "{path} is tracked; use discard instead"
                )));
            }
        }

        for path in paths {
            let target = self.root().join(path.trim_end_matches('/'));
            let result = if target.is_dir() {
                std::fs::remove_dir_all(&target)
            } else {
                std::fs::remove_file(&target)
            };
            result?;
        }
        Ok(())
    }
}
