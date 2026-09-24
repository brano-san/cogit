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
            Recovery::Branch { name, oid } => handle.create_branch(name, Some(oid), false)?,
            Recovery::Moved { name, oid } => handle.move_branch_back(name, oid)?,
            Recovery::Tag { name, oid } => handle.create_tag(&git_engine::TagRequest {
                name: name.clone(),
                target: Some(oid.clone()),
                message: None,
                force: false,
            })?,
            Recovery::Patch { patch, .. } => {
                handle.apply_patch_to(patch, false, git_engine::PatchTarget::WorkTree)?;
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
