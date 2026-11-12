//! The journal of destructive operations and the undo it carries. The means of undoing
//! never leaves this side of the boundary: it can be as large as the change itself.

use crate::{AppState, RepoId};
use serde::{Deserialize, Serialize};
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
    Discarded {
        kept: Vec<(String, String)>,
        left: Vec<(String, Option<String>)>,
    },
    /// The branch was deleted: undo creates it again, tracking what it tracked.
    Branch {
        name: String,
        oid: String,
        upstream: Option<String>,
    },
    /// The branch still exists and was moved (merge, rebase, cherry-pick): undo moves it
    /// back.
    Moved {
        name: String,
        oid: String,
        /// Where the branch stood right after the operation; `None` while it waits for
        /// Continue. Undo refuses once the branch has moved on.
        after: Option<String>,
        /// The other local branches the operation moved (`rebase.updateRefs`, G-06).
        stacked: Vec<StackedMove>,
    },
    /// HEAD was reset from `oid`; `branch` is `None` when it was detached. A hard reset
    /// also keeps the stash of what it threw away, taken on `oid`.
    Reset {
        branch: Option<String>,
        oid: String,
        mode: git_engine::ResetMode,
        stash: Option<String>,
        /// Untracked files the reset overwrote, kept in the object store.
        kept: Vec<(String, String)>,
        after: Option<String>,
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

/// A branch other than the checked-out one that moved with it. While the operation waits
/// for Continue every local branch is listed with `after: None`; settling keeps the moved.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StackedMove {
    pub name: String,
    pub oid: String,
    pub after: Option<String>,
}

/// Where HEAD, and for a rebase every local branch, stood before an operation.
pub(crate) struct Before {
    head: git_engine::Head,
    tips: Vec<(String, String)>,
}

impl Before {
    /// Merges, picks and the graph's own rebase plans move only the checked-out branch;
    /// listing the others would blame them for moves made elsewhere while one waits.
    pub(crate) fn head(handle: &git_engine::RepoHandle) -> Result<Self, git_engine::GitError> {
        Ok(Self {
            head: handle.head()?,
            tips: Vec::new(),
        })
    }

    /// A plain `git rebase` (also under pull and Split) honors `rebase.updateRefs`.
    pub(crate) fn with_branches(
        handle: &git_engine::RepoHandle,
    ) -> Result<Self, git_engine::GitError> {
        let head = handle.head()?;
        let tips = local_tips(handle).unwrap_or_default();
        Ok(Self { head, tips })
    }
}

fn local_tips(handle: &git_engine::RepoHandle) -> Option<Vec<(String, String)>> {
    handle
        .local_branch_tips()
        .inspect_err(|err| {
            tracing::error!(error = ?err, context = "branch tips unreadable; undo covers only the checked-out branch");
        })
        .ok()
}

/// The branches besides `current` that moved since `tips` were read; all of them, unsettled,
/// while the operation waits: a rebase moves the stacked ones only at its end.
fn stacked_since(
    handle: &git_engine::RepoHandle,
    current: &str,
    tips: Vec<(String, String)>,
    finished: bool,
) -> Vec<StackedMove> {
    let others = tips.into_iter().filter(|(name, _)| name != current);
    if !finished {
        return others
            .map(|(name, oid)| StackedMove {
                name,
                oid,
                after: None,
            })
            .collect();
    }
    let now: std::collections::HashMap<String, String> =
        local_tips(handle).unwrap_or_default().into_iter().collect();
    others
        .filter_map(|(name, oid)| {
            let after = now.get(&name).filter(|tip| **tip != oid)?.clone();
            Some(StackedMove {
                name,
                oid,
                after: Some(after),
            })
        })
        .collect()
}

impl Recovery {
    /// The backups (`refs/cogit/backup`) this recovery puts back.
    fn backups(&self) -> Vec<&str> {
        match self {
            Self::Stash { oid }
            | Self::Discard { stash: oid, .. }
            | Self::Worktree { stash: oid, .. } => {
                vec![oid.as_str()]
            }
            Self::Rollback { stash, .. } | Self::Reset { stash, .. } => {
                stash.as_deref().into_iter().collect()
            }
            Self::DroppedStash { entry } => vec![entry.oid.as_str()],
            Self::Branch { oid, .. } | Self::Tag { oid, .. } => vec![oid.as_str()],
            _ => Vec::new(),
        }
    }
}

/// What the journal shows. The means of undoing stays in `Undoable`, on this side of
/// the boundary: it can be as large as the change itself and the panel never reads it.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
    /// Lets go of a kept `oid` unless another entry of the journal (not `except`) still
    /// needs it, as two branches deleted at one commit do.
    pub(crate) fn unpin(
        &self,
        handle: &git_engine::RepoHandle,
        repo: RepoId,
        oid: &str,
        except: Option<u32>,
    ) {
        let shared = self.safety.read().iter().any(|held| {
            held.entry.repo == repo
                && Some(held.entry.id) != except
                && held.recovery.backups().contains(&oid)
        });
        if !shared {
            handle.forget_backup(oid);
        }
    }

    /// Newest first, like the Output panel.
    #[must_use]
    pub fn safety_log(&self) -> Vec<SafetyEntry> {
        let mut log: Vec<SafetyEntry> = self
            .safety
            .read()
            .iter()
            .rev()
            .map(|held| held.entry.clone())
            .collect();
        log.extend(self.remote_safety_log());
        log
    }

    pub fn undo_last(&self, repo: RepoId) -> Result<SafetyEntry, git_engine::GitError> {
        let held = self
            .safety
            .read()
            .iter()
            .rev()
            .find(|held| held.entry.repo == repo && held.entry.undoable)
            .cloned();
        match (held, self.remote(repo)) {
            (Some(held), _) => self.reverse(repo, held),
            (None, Some(remote)) => self.undo_through(repo, &remote, None),
            (None, None) => Err(git_engine::GitError::InvalidState(
                "nothing to undo".to_owned(),
            )),
        }
    }

    /// Any entry, not only the newest: the recoveries are independent restores rather than
    /// a stack, so the order is the user's to choose (T5.7).
    pub fn undo_entry(&self, repo: RepoId, id: u32) -> Result<SafetyEntry, git_engine::GitError> {
        let held = self
            .safety
            .read()
            .iter()
            .find(|held| held.entry.id == id && held.entry.repo == repo && held.entry.undoable)
            .cloned();
        match (held, self.remote(repo)) {
            (Some(held), _) => self.reverse(repo, held),
            (None, Some(remote)) => self.undo_through(repo, &remote, Some(id)),
            (None, None) => Err(git_engine::GitError::InvalidState(format!(
                "no undoable entry {id} here"
            ))),
        }
    }

    fn reverse(&self, repo: RepoId, held: Undoable) -> Result<SafetyEntry, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        match &held.recovery {
            Recovery::Stash { oid } => handle.stash_apply(oid)?,
            Recovery::DroppedStash { entry } => handle.restore_stash(entry)?,
            Recovery::Rollback { paths, stash } => {
                crate::rewrite::backup_rollback(&handle, paths, "cogit: before undoing a rollback")
                    .map_err(|err| crate::backup_failed("undoing the rollback of", &err))?;
                if let Some(oid) = stash {
                    handle.stash_apply(oid)?;
                }
            }
            // Applied over the staged side it would conflict with itself: both sides of
            // the merge add the same lines.
            Recovery::Discard { staged, stash } => {
                handle
                    .backup_paths(staged, "cogit: before undoing a discard")
                    .map_err(|err| crate::backup_failed("undoing the discard of", &err))?;
                handle.stash_apply(stash)?;
            }
            Recovery::Resolution { path, kept } => handle.unresolve(path, kept.as_deref())?,
            Recovery::Files { kept } => handle.write_back(kept)?,
            Recovery::Discarded { kept, left } => handle.write_back_over(kept, left)?,
            Recovery::Branch {
                name,
                oid,
                upstream,
            } => {
                handle.create_branch(name, Some(oid), false)?;
                // `branch -d` took the config section; a remote branch gone since stays gone.
                if let Some(upstream) = upstream
                    && let Err(err) = handle.set_upstream(name, Some(upstream))
                {
                    tracing::warn!(error = ?err, branch = %name, "the branch is back without its upstream");
                }
            }
            Recovery::Moved {
                name,
                oid,
                after,
                stacked,
            } => {
                wait_for_the_operation(&handle)?;
                let tip = tip_of(&handle, Some(name));
                ensure_unmoved(&handle, name, tip.as_deref(), after.as_deref())?;
                // Checked before anything moves: a refusal leaves every branch as it is.
                // Unsettled (`after: None`) are not known to have moved; deleted stay so.
                let mut others = Vec::new();
                for moved in stacked.iter().filter(|moved| moved.after.is_some()) {
                    let Some(now) = tip_of(&handle, Some(&moved.name)) else {
                        continue;
                    };
                    ensure_unmoved(&handle, &moved.name, Some(&now), moved.after.as_deref())?;
                    others.push((moved, now));
                }
                handle.move_branch_back(name, oid)?;
                let mut undone = Vec::new();
                let mut failed = None;
                for (moved, now) in others {
                    match handle.move_branch_back(&moved.name, &moved.oid) {
                        Ok(()) => undone.push(StackedMove {
                            name: moved.name.clone(),
                            oid: now,
                            after: Some(moved.oid.clone()),
                        }),
                        Err(err) => failed = failed.or(Some(err)),
                    }
                }
                self.record_undone(repo, &handle, Some(name), tip, oid, undone);
                if let Some(err) = failed {
                    self.safety
                        .write()
                        .retain(|kept| kept.entry.id != held.entry.id);
                    return Err(err);
                }
            }
            Recovery::Reset {
                branch,
                oid,
                mode,
                stash,
                kept,
                after,
            } => {
                wait_for_the_operation(&handle)?;
                let tip = tip_of(&handle, branch.as_deref());
                ensure_unmoved(
                    &handle,
                    branch.as_deref().unwrap_or("HEAD"),
                    tip.as_deref(),
                    after.as_deref(),
                )?;
                undo_reset(
                    &handle,
                    branch.as_deref(),
                    oid,
                    *mode,
                    stash.as_deref(),
                    kept,
                )?;
                self.record_undone(repo, &handle, branch.as_deref(), tip, oid, Vec::new());
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

        for oid in held.recovery.backups() {
            self.unpin(&handle, repo, oid, Some(held.entry.id));
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
    kept: &[(String, String)],
) -> Result<(), git_engine::GitError> {
    use git_engine::{Head, ResetMode};
    let on_it = match (handle.head()?, branch) {
        (Head::Branch { name, .. }, Some(branch)) => name == branch,
        (Head::Detached { .. }, None) => true,
        _ => false,
    };
    match (on_it, branch, stash.is_some() || !kept.is_empty()) {
        (true, ..) => {}
        (false, Some(branch), false) => return handle.move_branch_back(branch, oid),
        (false, Some(branch), true) => {
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
    if let Some(stash) = stash {
        handle.stash_apply(stash)?;
    }
    handle.write_back(kept)
}

/// Where the branch is now; HEAD's commit for `None`.
pub(crate) fn tip_of(handle: &git_engine::RepoHandle, branch: Option<&str>) -> Option<String> {
    match branch {
        Some(name) => handle.branch_tip(name).ok(),
        None => match handle.head() {
            Ok(git_engine::Head::Branch { oid, .. } | git_engine::Head::Detached { oid }) => {
                Some(oid)
            }
            _ => None,
        },
    }
}

/// A move or reset is reversed by putting the branch back, which also takes off whatever
/// was committed on it since: refused unless the branch is where the operation left it.
fn ensure_unmoved(
    handle: &git_engine::RepoHandle,
    label: &str,
    tip: Option<&str>,
    after: Option<&str>,
) -> Result<(), git_engine::GitError> {
    let (Some(tip), Some(after)) = (tip, after) else {
        return Ok(());
    };
    if tip == after {
        return Ok(());
    }
    let why = if handle.is_ancestor(after, tip).unwrap_or(false) {
        let n = handle.count_commits_between(after, tip).unwrap_or(0);
        format!("{label} has {n} commit(s) made since; undo would take them off it")
    } else {
        format!("{label} moved since; undo would discard where it went")
    };
    Err(git_engine::GitError::InvalidState(why))
}

impl AppState {
    /// A finished merge or rebase commits outside the journal's sight: the entry waiting
    /// for it learns where the branch ended up.
    pub(crate) fn settle_moves(&self, repo: RepoId) {
        let Ok(handle) = self.handle(repo) else {
            return;
        };
        // The branch has not reached its end while the operation waits (a rebase moves it
        // last); the entry keeps `after: None` until a commit or Continue finishes it.
        if handle
            .state()
            .is_ok_and(|state| state.is_interrupted_operation())
        {
            return;
        }
        let mut safety = self.safety.write();
        let waiting = safety.iter_mut().rev().find(|held| {
            held.entry.repo == repo && matches!(held.recovery, Recovery::Moved { after: None, .. })
        });
        if let Some(Undoable {
            recovery:
                Recovery::Moved {
                    name,
                    after,
                    stacked,
                    ..
                },
            ..
        }) = waiting
        {
            *after = tip_of(&handle, Some(name));
            if !stacked.is_empty() {
                let now: std::collections::HashMap<String, String> = local_tips(&handle)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                stacked.retain_mut(|moved| {
                    moved.after = now.get(&moved.name).cloned();
                    moved.after.as_ref().is_some_and(|tip| *tip != moved.oid)
                });
            }
        }
    }

    /// The undo is a move like any other, so it can be undone too.
    fn record_undone(
        &self,
        repo: RepoId,
        handle: &git_engine::RepoHandle,
        branch: Option<&str>,
        was: Option<String>,
        now: &str,
        stacked: Vec<StackedMove>,
    ) {
        let (Some(branch), Some(was)) = (branch, was) else {
            return;
        };
        self.record(
            repo,
            format!(
                "Undo: put {branch} back at {}{}",
                &now[..now.len().min(8)],
                also_moved(&stacked)
            ),
            Recovery::Moved {
                name: branch.to_owned(),
                oid: was,
                after: tip_of(handle, Some(branch)),
                stacked,
            },
        );
    }
}

/// The recovery of a branch that moved: `after` is read once the operation is over.
pub(crate) fn moved(
    handle: &git_engine::RepoHandle,
    name: String,
    oid: String,
    finished: bool,
) -> Recovery {
    let after = if finished {
        tip_of(handle, Some(&name))
    } else {
        None
    };
    Recovery::Moved {
        name,
        oid,
        after,
        stacked: Vec::new(),
    }
}

/// `moved` for the checked-out branch and every other one the operation took along.
pub(crate) fn moved_from(
    handle: &git_engine::RepoHandle,
    before: Before,
    finished: bool,
) -> Recovery {
    let git_engine::Head::Branch { name, oid } = before.head else {
        return Recovery::None;
    };
    let stacked = stacked_since(handle, &name, before.tips, finished);
    let after = finished.then(|| tip_of(handle, Some(&name))).flatten();
    Recovery::Moved {
        name,
        oid,
        after,
        stacked,
    }
}

pub(crate) fn also_moved_by(recovery: &Recovery) -> String {
    match recovery {
        Recovery::Moved { stacked, .. } => also_moved(stacked),
        _ => String::new(),
    }
}

/// `" (also moved a, b)"` for the journal line; empty when nothing else moved.
fn also_moved(stacked: &[StackedMove]) -> String {
    let names: Vec<&str> = stacked
        .iter()
        .filter(|moved| moved.after.is_some())
        .map(|moved| moved.name.as_str())
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(" (also moved {})", names.join(", "))
    }
}
