//! The journal of destructive operations and the undo it carries. The means of undoing
//! never leaves this side of the boundary: it can be as large as the change itself.

use crate::{AppState, RepoId};
use serde::Serialize;
use std::sync::atomic::Ordering;

/// What has to be put back to reverse one destructive operation (INV-12).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Recovery {
    Stash {
        oid: String,
    },
    /// A stash taken off the list: undo lists it again at the place it had, and leaves the
    /// working tree alone.
    DroppedStash {
        entry: git_engine::StashEntry,
    },
    /// A past version written over the paths. Undo stashes it away, as a discard keeps
    /// what it removes, then applies the stash of the work it replaced, if there was any.
    Rollback {
        paths: Vec<String>,
        stash: Option<String>,
    },
    /// Unstaged changes discarded from paths that keep their staged side: undo stashes
    /// those paths away, as a rollback's undo does, then applies the stash of the discard.
    Discard {
        staged: Vec<String>,
        stash: String,
    },
    /// A conflict resolved over the working file: undo recreates the conflict where git
    /// can and writes the file back as it was, hand edits included.
    Resolution {
        path: String,
        kept: Option<String>,
    },
    /// Files deleted to the bin, each kept in the object store: undo writes them back.
    Files {
        kept: Vec<(String, String)>,
    },
    /// The branch was deleted: undo creates it again.
    Branch {
        name: String,
        oid: String,
    },
    /// The branch still exists and was moved (merge, rebase, cherry-pick): undo moves it
    /// back.
    Moved {
        name: String,
        oid: String,
    },
    /// HEAD was reset from `oid`; `branch` is `None` when it was detached. A hard reset
    /// also keeps the stash of what it threw away, taken on `oid`.
    Reset {
        branch: Option<String>,
        oid: String,
        mode: git_engine::ResetMode,
        stash: Option<String>,
    },
    Tag {
        name: String,
        oid: String,
    },
    /// The patch that was reversed. Undo applies it again, which puts back exactly the
    /// lines that went and leaves the rest of the file alone.
    Patch {
        path: String,
        patch: String,
    },
    /// A worktree removed by force: undo adds it back where it was, on its branch or its
    /// commit, and applies the stash of its changes there, not in the owner's tree.
    Worktree {
        path: String,
        checkout: String,
        stash: String,
    },
    /// Recorded for the journal, refused by undo: honesty beats a half-working restore.
    None,
}

/// What the journal shows. The means of undoing stays in `Undoable`, on this side of
/// the boundary: it can be as large as the change itself and the panel never reads it.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SafetyEntry {
    pub id: u32,
    pub repo: RepoId,
    pub description: String,
    pub undoable: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Undoable {
    pub(crate) entry: SafetyEntry,
    recovery: Recovery,
}

impl AppState {
    /// Newest first, like the Output panel.
    #[must_use]
    pub fn safety_log(&self) -> Vec<SafetyEntry> {
        self.safety
            .read()
            .iter()
            .rev()
            .map(|held| held.entry.clone())
            .collect()
    }

    pub fn undo_last(&self, repo: RepoId) -> Result<SafetyEntry, git_engine::GitError> {
        let held = self
            .safety
            .read()
            .iter()
            .rev()
            .find(|held| held.entry.repo == repo && held.entry.undoable)
            .cloned()
            .ok_or_else(|| git_engine::GitError::InvalidState("nothing to undo".to_owned()))?;
        self.reverse(repo, held)
    }

    /// Any entry, not only the newest: the recoveries are independent restores rather than
    /// a stack, so the order is the user's to choose (T5.7).
    pub fn undo_entry(&self, repo: RepoId, id: u32) -> Result<SafetyEntry, git_engine::GitError> {
        let held = self
            .safety
            .read()
            .iter()
            .find(|held| held.entry.id == id && held.entry.repo == repo && held.entry.undoable)
            .cloned()
            .ok_or_else(|| {
                git_engine::GitError::InvalidState(format!("no undoable entry {id} here"))
            })?;
        self.reverse(repo, held)
    }

    fn reverse(&self, repo: RepoId, held: Undoable) -> Result<SafetyEntry, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        match &held.recovery {
            Recovery::Stash { oid } => handle.stash_apply(oid)?,
            Recovery::DroppedStash { entry } => handle.restore_stash(entry)?,
            Recovery::Rollback { paths, stash } => {
                handle
                    .stash_paths(paths, "cogit: before undoing a rollback")
                    .map_err(|err| crate::backup_failed("undoing the rollback of", &err))?;
                if let Some(oid) = stash {
                    handle.stash_apply(oid)?;
                }
            }
            // Applied over the staged side it would conflict with itself: both sides of
            // the merge add the same lines.
            Recovery::Discard { staged, stash } => {
                handle
                    .stash_paths(staged, "cogit: before undoing a discard")
                    .map_err(|err| crate::backup_failed("undoing the discard of", &err))?;
                handle.stash_apply(stash)?;
            }
            Recovery::Resolution { path, kept } => handle.unresolve(path, kept.as_deref())?,
            Recovery::Files { kept } => handle.write_back(kept)?,
            Recovery::Branch { name, oid } => handle.create_branch(name, Some(oid), false)?,
            Recovery::Moved { name, oid } => {
                wait_for_the_operation(&handle)?;
                handle.move_branch_back(name, oid)?;
            }
            Recovery::Reset {
                branch,
                oid,
                mode,
                stash,
            } => {
                wait_for_the_operation(&handle)?;
                undo_reset(&handle, branch.as_deref(), oid, *mode, stash.as_deref())?;
            }
            Recovery::Tag { name, oid } => handle.create_tag(&git_engine::TagRequest {
                name: name.clone(),
                target: Some(oid.clone()),
                message: None,
                force: false,
            })?,
            Recovery::Patch { patch, .. } => {
                handle.apply_patch_to(patch, false, git_engine::PatchTarget::WorkTree)?;
            }
            Recovery::Worktree {
                path,
                checkout,
                stash,
            } => {
                handle.add_worktree(path, checkout, false)?;
                git_engine::RepoHandle::open_exact(std::path::Path::new(path))?
                    .stash_apply(stash)?;
            }
            Recovery::None => {
                return Err(git_engine::GitError::InvalidState(
                    "this operation cannot be undone".to_owned(),
                ));
            }
        }

        self.safety
            .write()
            .retain(|kept| kept.entry.id != held.entry.id);
        Ok(held.entry)
    }

    pub(crate) fn record(&self, repo: RepoId, description: String, recovery: Recovery) {
        let entry = SafetyEntry {
            id: self.next_entry_id.fetch_add(1, Ordering::Relaxed),
            repo,
            description,
            undoable: !matches!(recovery, Recovery::None),
        };
        tracing::info!(
            repo = repo.0,
            entry = %entry.description,
            undoable = entry.undoable,
            "destructive operation recorded"
        );
        // Checked under the journal's lock: close clears the journal after unregistering,
        // so an entry is either cleared by it or never pushed.
        let mut safety = self.safety.write();
        if self.repos.read().contains_key(&repo) {
            safety.push(Undoable { entry, recovery });
        } else {
            tracing::info!(
                repo = repo.0,
                "the repository closed meanwhile; not kept for Undo"
            );
        }
    }
}

/// Git refuses to move a branch under a stopped merge or rebase and says nothing of what to
/// do; aborting on the user's behalf would throw away what they have resolved so far.
fn wait_for_the_operation(handle: &git_engine::RepoHandle) -> Result<(), git_engine::GitError> {
    if handle.state()?.is_interrupted_operation() {
        return Err(git_engine::GitError::InvalidState(
            "an operation is in progress: continue or abort it, then undo".to_owned(),
        ));
    }
    Ok(())
}

/// Soft and mixed never touched the files, so the same mode reverses them exactly; the
/// others go back through `keep`, which refuses rather than overwrites what changed since.
/// A hard reset's stash was taken on the old tip, so it goes back once HEAD is there.
fn undo_reset(
    handle: &git_engine::RepoHandle,
    branch: Option<&str>,
    oid: &str,
    mode: git_engine::ResetMode,
    stash: Option<&str>,
) -> Result<(), git_engine::GitError> {
    use git_engine::{Head, ResetMode};
    let on_it = match (handle.head()?, branch) {
        (Head::Branch { name, .. }, Some(branch)) => name == branch,
        (Head::Detached { .. }, None) => true,
        _ => false,
    };
    match (on_it, branch, stash) {
        (true, ..) => {}
        (false, Some(branch), None) => return handle.move_branch_back(branch, oid),
        (false, Some(branch), Some(_)) => {
            return Err(git_engine::GitError::InvalidState(format!(
                "check out {branch} to undo its reset: the changes it saved belong there"
            )));
        }
        (false, None, _) => {
            return Err(git_engine::GitError::InvalidState(
                "HEAD is on a branch now; the reset of the detached HEAD cannot be undone here"
                    .to_owned(),
            ));
        }
    }
    let back = match mode {
        ResetMode::Soft | ResetMode::Mixed => mode,
        ResetMode::Hard | ResetMode::Keep | ResetMode::Merge => ResetMode::Keep,
    };
    handle.reset(oid, back)?;
    stash.map_or(Ok(()), |stash| handle.stash_apply(stash))
}
