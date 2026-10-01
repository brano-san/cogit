use crate::{
    FileEntry, FileMode, FileStatus, PushCommit, RepoHandle, Result, SubmoduleChange, WorktreeView,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

/// How many commit names a submodule row carries; the total is counted past it.
const NAMED: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSubmodules {
    /// Every submodule checked out in the worktree: removing it deletes their repositories.
    pub paths: Vec<String>,
    /// The ones with something to lose in the parent's eyes, as rows of the changes list.
    pub changed: Vec<FileEntry>,
}

/// Commits a submodule holds that no remote has: removing the worktree deletes the only copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnpushedInSubmodule {
    pub path: String,
    pub total: u32,
    pub commits: Vec<PushCommit>,
}

#[allow(missing_debug_implementations)] // holds a closure
/// A validated worktree folder the three stages of the scan read independently, each
/// through a handle of its own: a `gix` repository cannot be shared between threads.
#[derive(Clone)]
pub struct WorktreeScan {
    folder: PathBuf,
    stop: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl RepoHandle {
    pub fn worktree_scan(&self, path: &str) -> Result<WorktreeScan> {
        // `linked_handle` refuses anything git does not list, and a missing folder.
        drop(self.linked_handle(path)?);
        Ok(WorktreeScan {
            folder: PathBuf::from(path),
            stop: Arc::new(|| false),
        })
    }
}

impl WorktreeScan {
    /// Checked between submodules: a stage that is not wanted any more gives up early.
    #[must_use]
    pub fn stopping_on(mut self, stop: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.stop = Arc::new(stop);
        self
    }

    fn open(&self) -> Result<RepoHandle> {
        RepoHandle::open_exact(&self.folder)
    }

    /// Edits and untracked files; a submodule's own status is the next stage's.
    pub fn changes(&self) -> Result<Vec<FileEntry>> {
        let files = self.open()?.files_with(WorktreeView::default(), true)?;
        let mut seen = std::collections::BTreeSet::new();
        Ok(files
            .staged
            .into_iter()
            .chain(files.unstaged)
            .filter(|file| seen.insert(file.path.clone()))
            .collect())
    }

    pub fn submodules(&self) -> Result<WorktreeSubmodules> {
        let modules = checked_out(&self.open()?)?;
        let rows = parallel(&modules, |(path, recorded)| {
            if (self.stop)() {
                return None;
            }
            let inside = RepoHandle::open_exact(&self.folder.join(path)).ok()?;
            change_of(&inside, path, recorded)
        });
        Ok(WorktreeSubmodules {
            paths: modules.into_iter().map(|(path, _)| path).collect(),
            changed: rows.into_iter().flatten().collect(),
        })
    }

    pub fn unpushed(&self) -> Result<Vec<UnpushedInSubmodule>> {
        let modules = checked_out(&self.open()?)?;
        let found = parallel(&modules, |(path, _)| {
            if (self.stop)() {
                return None;
            }
            let inside = RepoHandle::open_exact(&self.folder.join(path)).ok()?;
            let (total, commits) = inside.unpushed_anywhere();
            (total > 0).then(|| UnpushedInSubmodule {
                path: path.clone(),
                total,
                commits,
            })
        });
        Ok(found.into_iter().flatten().collect())
    }
}

/// Gitlinks of the index whose repository is in the folder, with the commit recorded.
fn checked_out(handle: &RepoHandle) -> Result<Vec<(String, gix::ObjectId)>> {
    let index = handle.current_index()?;
    Ok(index
        .entries()
        .iter()
        .filter(|entry| entry.mode.is_submodule())
        .map(|entry| (entry.path(&index).to_string(), entry.id))
        .filter(|(path, _)| handle.root().join(path).join(".git").exists())
        .collect())
}

fn change_of(inside: &RepoHandle, path: &str, recorded: &gix::ObjectId) -> Option<FileEntry> {
    let new_commits = match inside.head().ok()? {
        crate::Head::Branch { oid, .. } | crate::Head::Detached { oid } => oid != *recorded,
        crate::Head::Unborn { .. } => false,
    };
    let files = inside.worktree_files().ok()?;
    let mut change = SubmoduleChange {
        new_commits,
        ..Default::default()
    };
    for file in files.staged.iter().chain(&files.unstaged) {
        if file.status == FileStatus::Untracked {
            change.untracked = true;
        } else {
            change.modified = true;
        }
    }
    (change.new_commits || change.modified || change.untracked).then(|| FileEntry {
        path: path.to_owned(),
        old_path: None,
        status: FileStatus::Modified,
        mode: FileMode::Submodule,
        mode_change: None,
        similarity: None,
        submodule: Some(change),
        conflict: None,
    })
}

impl RepoHandle {
    /// Commits reachable from HEAD or a local branch and from no remote-tracking ref.
    fn unpushed_anywhere(&self) -> (u32, Vec<PushCommit>) {
        let tips_under = |prefix: &str| -> Vec<gix::ObjectId> {
            let Ok(platform) = self.repo.references() else {
                return Vec::new();
            };
            let Ok(found) = platform.prefixed(prefix) else {
                return Vec::new();
            };
            found
                .filter_map(std::result::Result::ok)
                .filter_map(|mut reference| reference.peel_to_id().ok())
                .map(gix::Id::detach)
                .collect()
        };
        let mut tips = tips_under("refs/heads/");
        tips.extend(self.repo.head_id().ok().map(gix::Id::detach));
        let hidden = tips_under("refs/remotes/");
        let walk = match self.repo.rev_walk(tips).with_hidden(hidden).all() {
            Ok(walk) => walk,
            Err(err) => {
                tracing::error!(error = ?err, context = "unpushed commits of a submodule");
                return (0, Vec::new());
            }
        };
        let mut total = 0_u32;
        let mut commits = Vec::new();
        for info in walk.filter_map(std::result::Result::ok) {
            total = total.saturating_add(1);
            if commits.len() < NAMED {
                let summary = info
                    .object()
                    .ok()
                    .and_then(|commit| commit.message().ok().map(|m| m.summary().to_string()))
                    .unwrap_or_default();
                commits.push(PushCommit {
                    oid: info.id.to_string(),
                    summary,
                });
            }
        }
        (total, commits)
    }
}

/// `items.map(f)` with one thread per item, order kept.
fn parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let running: Vec<_> = items.iter().map(|item| scope.spawn(|| f(item))).collect();
        running
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect()
    })
}
