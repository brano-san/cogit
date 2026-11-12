use crate::{GitError, RepoHandle, Result};
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::EntryStatus;
use gix::status::{Item, UntrackedFiles};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    /// Submodules with edits or new files inside at their recorded commit (`S.<m><u>`). A
    /// moved one is in `unstaged` and not here; `is_clean` ignores these.
    pub dirty_submodules: u32,
}

/// The counters and the conflicted paths, from one read of the status (R-316).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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
        past_broken_submodules(self, |skip| self.working_state_reading(skip))
    }

    fn working_state_reading(&self, skip_submodules: bool) -> Result<WorkingState> {
        let mut status = RepoStatus::default();
        let mut conflicted = Vec::new();
        for item in self.status_items(skip_submodules)? {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            status.dirty_submodules += u32::from(crate::worktree::changed_submodule(&item));
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
        self.changed(true)
    }

    /// Staged or unstaged changes to tracked files: what git will not pull over.
    pub fn has_tracked_changes(&self) -> Result<bool> {
        self.changed(false)
    }

    fn changed(&self, untracked: bool) -> Result<bool> {
        if self.repo.is_bare() {
            return Ok(false);
        }
        past_broken_submodules(self, |skip| self.changed_reading(untracked, skip))
    }

    fn changed_reading(&self, untracked: bool, skip_submodules: bool) -> Result<bool> {
        for item in self.status_items(skip_submodules)? {
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
                    untracked && matches!(entry.status, gix::dir::entry::Status::Untracked)
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
        past_broken_submodules(self, |skip| self.change_counts_reading(skip))
    }

    fn change_counts_reading(&self, skip_submodules: bool) -> Result<(u32, u32)> {
        let mut changed = std::collections::HashSet::new();
        let mut untracked = 0;
        for item in self.status_items(skip_submodules)? {
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
    fn status_items(&self, skip_submodules: bool) -> Result<gix::status::Iter> {
        let mut platform = self
            .status_platform()?
            .untracked_files(UntrackedFiles::Files);
        if skip_submodules {
            platform = platform.index_worktree_submodules(gix::status::Submodule::Given {
                ignore: gix::submodule::config::Ignore::All,
                check_dirty: false,
            });
        }
        platform
            .into_iter(None::<gix::bstr::BString>)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))
    }
}

type Stamp = Vec<Option<std::time::SystemTime>>;
type BrokenSubmodules = std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Stamp>>;

fn broken_submodules() -> &'static BrokenSubmodules {
    static BROKEN: std::sync::OnceLock<BrokenSubmodules> = std::sync::OnceLock::new();
    BROKEN.get_or_init(Default::default)
}

pub(crate) fn past_broken_submodules<T>(
    repo: &RepoHandle,
    read: impl Fn(bool) -> Result<T>,
) -> Result<T> {
    skipping_broken_submodules(repo.root(), || stamp(&watched_paths(repo)), read)
}

/// Repairing a submodule writes only inside it, its `.git/modules` entry or `.git/config`,
/// none of which the superproject's index sees, so each of them ends the skip.
fn watched_paths(repo: &RepoHandle) -> Vec<std::path::PathBuf> {
    let mut watched = vec![
        repo.repo.index_path(),
        repo.root().join(".gitmodules"),
        repo.repo.common_dir().join("config"),
    ];
    let modules = match repo.repo.submodules() {
        Ok(modules) => modules.into_iter().flatten(),
        Err(err) => {
            tracing::error!(error = ?err, context = "submodules for the broken-submodule stamp");
            return watched;
        }
    };
    for module in modules {
        if let Ok(work_dir) = module.work_dir() {
            watched.push(work_dir.join(".git"));
        }
        if let Ok(git_dir) = module.git_dir() {
            watched.push(git_dir.join("HEAD"));
            watched.push(git_dir);
        }
    }
    watched
}

fn stamp(watched: &[std::path::PathBuf]) -> Stamp {
    watched
        .iter()
        .map(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
        })
        .collect()
}

pub(crate) fn forget_broken_submodules(root: &std::path::Path) {
    broken_submodules()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(root);
}

/// The stamp is taken only once a submodule failed: listing submodules costs a
/// `.gitmodules` read the healthy path need not pay.
fn skipping_broken_submodules<T>(
    root: &std::path::Path,
    stamp: impl Fn() -> Stamp,
    read: impl Fn(bool) -> Result<T>,
) -> Result<T> {
    let known = broken_submodules()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(root)
        .cloned();
    if known.is_some_and(|known| known == stamp()) {
        return read(true);
    }
    match read(false) {
        Err(GitError::Internal(message)) if message.contains("for submodule at") => {
            tracing::warn!(
                error = %message,
                "a submodule's status cannot be read; the rest is read without submodules"
            );
            broken_submodules()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(root.to_path_buf(), stamp());
            read(true)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_failed_submodule_status_is_skipped_until_the_index_changes() {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("index");
        std::fs::write(&index, "1").unwrap();
        let calls = RefCell::new(Vec::new());
        let read = |skip: bool| {
            calls.borrow_mut().push(skip);
            if skip {
                Ok(())
            } else {
                Err(GitError::Internal(
                    "Could not determine status for submodule at 'x'".to_owned(),
                ))
            }
        };

        let watched = || stamp(std::slice::from_ref(&index));
        skipping_broken_submodules(dir.path(), watched, read).unwrap();
        skipping_broken_submodules(dir.path(), watched, read).unwrap();
        assert_eq!(*calls.borrow(), [false, true, true]);

        std::fs::File::options()
            .write(true)
            .open(&index)
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        skipping_broken_submodules(dir.path(), watched, read).unwrap();
        assert_eq!(*calls.borrow(), [false, true, true, false, true]);
    }
}
