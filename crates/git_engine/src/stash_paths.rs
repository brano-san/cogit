//! Applying only some files of a stash (F-945).

use crate::{GitError, Head, RepoHandle, Result};
use std::collections::BTreeSet;

/// The chosen files of a stash, sorted by where they come from and checked before any write.
#[derive(Debug, Clone)]
pub struct StashPick {
    pub index: u32,
    /// HEAD when it was checked: Undo puts the index of the paths back to it.
    pub head: String,
    stash: String,
    staged_rev: String,
    untracked_rev: Option<String>,
    written: Vec<String>,
    /// Of those, the ones HEAD lacks: `stash apply` keeps them added without `--index`.
    added: Vec<String>,
    deleted: Vec<String>,
    untracked: Vec<String>,
    paths: Vec<String>,
}

impl StashPick {
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

fn holds(tree: &gix::Tree<'_>, path: &str) -> Result<bool> {
    let entry = tree
        .lookup_entry_by_path(path)
        .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?;
    Ok(entry.is_some_and(|entry| !entry.mode().is_tree()))
}

fn under(busy: &BTreeSet<String>, path: &str) -> bool {
    busy.contains(path)
        || path
            .match_indices('/')
            .any(|(at, _)| busy.contains(&path[..=at]) || busy.contains(&path[..at]))
}

impl RepoHandle {
    /// Refuses, before anything is written, a path the stash lacks or one whose local state
    /// it would overwrite: changed, staged, untracked or ignored on disk.
    pub fn stash_pick(&self, oid: &str, paths: &[String]) -> Result<StashPick> {
        let paths: Vec<String> = paths
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if paths.is_empty() {
            return Err(GitError::InvalidState(
                "no files chosen to apply".to_owned(),
            ));
        }
        let index = self.stash_by_oid(oid)?.index;
        let head = match self.head()? {
            Head::Branch { oid, .. } | Head::Detached { oid } => oid,
            Head::Unborn { .. } => {
                return Err(GitError::InvalidState(
                    "HEAD has no commit yet: apply the whole stash instead".to_owned(),
                ));
            }
        };
        let commit = self.find_commit(oid)?;
        let parents: Vec<gix::ObjectId> = commit.parent_ids().map(|id| id.detach()).collect();
        let (Some(&base), Some(&staged)) = (parents.first(), parents.get(1)) else {
            return Err(GitError::InvalidState(format!("{oid} is not a stash")));
        };
        let untracked_rev = parents.get(2).copied();
        let stash_tree = self.tree_of(commit.id)?;
        let base_tree = self.tree_of(base)?;
        let staged_tree = self.tree_of(staged)?;
        let untracked_tree = untracked_rev.map(|id| self.tree_of(id)).transpose()?;
        let head_tree = self
            .find_commit(&head)?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of HEAD: {err}")))?;

        let mut pick = StashPick {
            index,
            head,
            stash: oid.to_owned(),
            staged_rev: staged.to_string(),
            untracked_rev: untracked_rev.map(|id| id.to_string()),
            written: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            untracked: Vec::new(),
            paths: paths.clone(),
        };
        let mut missing = Vec::new();
        for path in paths {
            if let Some(tree) = &untracked_tree
                && holds(tree, &path)?
            {
                pick.untracked.push(path);
            } else if holds(&stash_tree, &path)? {
                if !holds(&head_tree, &path)? {
                    pick.added.push(path.clone());
                }
                pick.written.push(path);
            } else if holds(&base_tree, &path)? || holds(&staged_tree, &path)? {
                pick.deleted.push(path);
            } else {
                missing.push(path);
            }
        }
        if !missing.is_empty() {
            return Err(GitError::InvalidState(format!(
                "stash@{{{index}}} does not hold {}",
                missing.join(", ")
            )));
        }
        self.refuse_local_changes(&pick.paths)?;
        Ok(pick)
    }

    fn refuse_local_changes(&self, paths: &[String]) -> Result<()> {
        let files = self.worktree_files()?;
        let busy: BTreeSet<String> = files
            .staged
            .iter()
            .chain(&files.unstaged)
            .flat_map(|entry| std::iter::once(entry.path.clone()).chain(entry.old_path.clone()))
            .collect();
        let index = self.current_index()?;
        let mut changed = Vec::new();
        for path in paths {
            // Ignored files are not listed, and a file the index lacks is not git's to write over.
            let in_the_way = under(&busy, path)
                || (index.entry_by_path(path.as_str().into()).is_none()
                    && std::fs::symlink_metadata(self.root().join(path)).is_ok());
            if in_the_way {
                changed.push(path.as_str());
            }
        }
        if changed.is_empty() {
            return Ok(());
        }
        Err(GitError::InvalidState(format!(
            "Nothing was applied: local changes to {} would be overwritten. Commit, stash or \
             discard them first, or leave these files out.",
            changed.join(", ")
        )))
    }

    /// Each of `paths` as on disk, kept in the object store; `None` where there is no file.
    pub fn keep_present(&self, paths: &[String]) -> Result<Vec<(String, Option<String>)>> {
        let present: Vec<String> = paths
            .iter()
            .filter(|path| self.root().join(path).is_file())
            .cloned()
            .collect();
        let mut kept = self.keep_files(&present)?.into_iter();
        Ok(paths
            .iter()
            .map(|path| {
                let oid = present.contains(path).then(|| kept.next()).flatten();
                (path.clone(), oid)
            })
            .collect())
    }

    /// Writes what `stash_pick` sorted. Without `restore_index` the index ends as `stash
    /// apply` leaves it: at HEAD, files new to it added.
    pub fn apply_stash_pick(&self, pick: &StashPick, restore_index: bool) -> Result<()> {
        if !pick.written.is_empty() {
            self.run_git_paths(&["checkout", &pick.stash], &pick.written)?;
        }
        if !pick.deleted.is_empty() {
            self.run_git_paths(&["rm", "-q", "--ignore-unmatch"], &pick.deleted)?;
        }
        if let Some(rev) = &pick.untracked_rev
            && !pick.untracked.is_empty()
        {
            let source = format!("--source={rev}");
            self.run_git_paths(&["restore", &source, "--worktree"], &pick.untracked)?;
        }
        let tracked: Vec<String> = pick.written.iter().chain(&pick.deleted).cloned().collect();
        if tracked.is_empty() {
            return Ok(());
        }
        let to = if restore_index {
            &pick.staged_rev
        } else {
            &pick.head
        };
        self.run_git_paths(&["reset", "-q", to], &tracked)?;
        if !restore_index && !pick.added.is_empty() {
            self.run_git_paths(&["add"], &pick.added)?;
        }
        Ok(())
    }

    /// Refused if any path changed since (`left`); the index of the paths goes back to `head`.
    pub fn undo_stash_pick(
        &self,
        head: &str,
        kept: &[(String, Option<String>)],
        left: &[(String, Option<String>)],
    ) -> Result<()> {
        let paths: Vec<String> = left.iter().map(|(path, _)| path.clone()).collect();
        let now = self.file_hashes(&paths)?;
        if let Some(((path, _), _)) = left.iter().zip(&now).find(|((_, was), now)| was != *now) {
            return Err(GitError::InvalidState(format!(
                "{path} changed since the stash was applied: Undo would write over it"
            )));
        }
        let mut back = Vec::new();
        for (path, oid) in kept {
            match oid {
                Some(oid) => back.push((path.clone(), oid.clone())),
                None => match std::fs::remove_file(self.root().join(path)) {
                    Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                        return Err(err.into());
                    }
                    _ => {}
                },
            }
        }
        self.write_kept(&back)?;
        let paths: Vec<String> = kept.iter().map(|(path, _)| path.clone()).collect();
        self.run_git_paths(&["reset", "-q", head], &paths).map(drop)
    }
}
