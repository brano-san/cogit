use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub index: u32,
    pub oid: String,
    pub message: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StashOptions {
    pub message: String,
    pub include_untracked: bool,
    pub keep_index: bool,
}

/// The three things `git stash` puts away, each readable without touching the working tree.
/// A stash is a commit: `^1` is HEAD at the time, `^2` the index, `^3` the untracked files.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StashContents {
    pub worktree: Vec<crate::FileEntry>,
    pub index: Vec<crate::FileEntry>,
    pub untracked: Vec<crate::FileEntry>,
    /// Revisions the caller hands back to `diff_file` as `CommitVsCommit`.
    pub base: String,
    pub worktree_rev: String,
    pub index_rev: String,
    pub untracked_rev: Option<String>,
}

/// Where the copies Undo puts back are kept: out of `refs/stash`, so the user's list, its
/// numbers and `git stash pop` never meet them (R-514).
pub const BACKUP_REFS: &str = "refs/cogit/backup/";

impl RepoHandle {
    /// `stash_paths` whose stash is kept as a backup for Undo instead of listed.
    pub fn backup_paths(&self, paths: &[String], message: &str) -> Result<Option<String>> {
        let made = self.stash_paths(paths, message)?;
        self.keep_as_backup(made)
    }

    /// `stash_before_reset`, kept as a backup for Undo instead of listed.
    pub fn backup_before_reset(&self, message: &str) -> Result<Option<String>> {
        let made = self.stash_before_reset(message)?;
        self.keep_as_backup(made)
    }

    /// The stash just made on top of the list moves to a ref of its own, and the list is
    /// left as the user had it. The ref comes first: a failed drop only lists it as well.
    /// Written by gix, not by a `git` of its own: Cogit's own namespace needs no hook, no
    /// credential and no merge, and a discard stays two processes, not three (R-24, R-514).
    fn keep_as_backup(&self, made: Option<String>) -> Result<Option<String>> {
        let Some(oid) = made else {
            return Ok(None);
        };
        let id = gix::ObjectId::from_hex(oid.as_bytes())
            .map_err(|err| GitError::Internal(format!("git gave a stash id {oid}: {err}")))?;
        self.repo
            .reference(
                format!("{BACKUP_REFS}{oid}").as_str(),
                id,
                gix::refs::transaction::PreviousValue::Any,
                "cogit: kept for Undo",
            )
            .map_err(|err| GitError::Internal(format!("cannot keep {oid} for Undo: {err}")))?;
        if self.stash_top().as_deref() == Some(oid.as_str()) {
            self.run_git(&["stash", "drop", "--quiet", "stash@{0}"])?;
        }
        Ok(Some(oid))
    }

    /// Undo put it back, so nothing is left to keep it for; a failure only leaves it kept.
    pub fn forget_backup(&self, oid: &str) {
        let name = format!("{BACKUP_REFS}{oid}");
        let gone = self
            .repo
            .find_reference(name.as_str())
            .map_err(|err| err.to_string())
            .and_then(|reference| reference.delete().map_err(|err| err.to_string()));
        if let Err(err) = gone {
            tracing::warn!(error = %err, oid, "a backup for Undo stays kept");
        }
    }

    pub fn stash_paths(&self, paths: &[String], message: &str) -> Result<Option<String>> {
        // Git hands the list on to a `git clean` of its own, on a command line: past its
        // limit the stash is made and the files stay (R-191).
        if !crate::runner::fits_command_line(paths) {
            return Err(GitError::InvalidState(format!(
                "{} paths are too many to stash at once",
                paths.len()
            )));
        }
        let paths = &self.stashable(paths)?;
        if paths.is_empty() {
            return Ok(None);
        }
        let before = self.stash_top();

        let mut args = vec!["stash", "push", "--include-untracked"];
        if !message.trim().is_empty() {
            args.extend(["--message", message]);
        }
        let unmerged = self.unmerged_paths()?;
        if unmerged.is_empty() {
            self.run_git_paths(&args, paths)?;
        } else {
            self.push_beside_a_conflict(&args, &unmerged, Some(paths))?;
            // What push does to the index at the paths, had it been let (R-441).
            self.run_git_paths(&["reset", "-q"], paths)?;
        }

        let after = self.stash_top();
        Ok(if after == before { None } else { after })
    }

    /// The backup of a hard reset: the tracked changes, even beside a conflict — resetting
    /// is how a stopped merge is left behind, and it throws those away.
    pub fn stash_before_reset(&self, message: &str) -> Result<Option<String>> {
        let unmerged = self.unmerged_paths()?;
        if unmerged.is_empty() {
            return self.stash_push_if_any(&StashOptions {
                message: message.to_owned(),
                include_untracked: false,
                keep_index: false,
            });
        }
        let before = self.stash_top();
        self.push_beside_a_conflict(&["stash", "push", "--message", message], &unmerged, None)?;
        let after = self.stash_top();
        Ok(if after == before { None } else { after })
    }

    /// `stash push` refuses while any index entry is unmerged ("needs merge"), whatever the
    /// paths: it rewrites the index. It runs on a scratch copy then, those entries reset to
    /// HEAD; the conflicted files go into the stash as changes to them.
    fn push_beside_a_conflict(
        &self,
        push: &[&str],
        unmerged: &[String],
        paths: Option<&[String]>,
    ) -> Result<()> {
        const FROM_STDIN: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];
        let scratch = crate::commit_write::Scratch::beside_index(self, "stash");
        std::fs::copy(self.repo.index_path(), &scratch.0)
            .map_err(|err| GitError::Io(format!("cannot copy the index: {err}")))?;
        let reset = [&["reset", "-q"][..], &FROM_STDIN[..]].concat();
        self.run_git_indexed(&scratch.0, &reset, Some(unmerged.join("\0").as_bytes()))?;
        match paths {
            Some(paths) => self.run_git_indexed(
                &scratch.0,
                &[push, &FROM_STDIN[..]].concat(),
                Some(paths.join("\0").as_bytes()),
            ),
            None => self.run_git_indexed(&scratch.0, push, None),
        }
        .map(drop)
    }

    /// Those of `paths` a stash of paths can take. `stash push` hands them to a `git add` of
    /// its own, which refuses one in neither the index nor the folder — a staged deletion —
    /// after the stash is made, and leaves the edits where they were (R-486).
    pub fn stashable(&self, paths: &[String]) -> Result<Vec<String>> {
        let mut index = None;
        let mut kept = Vec::with_capacity(paths.len());
        for path in paths {
            if std::fs::symlink_metadata(self.root().join(path)).is_ok() {
                kept.push(path.clone());
                continue;
            }
            if index.is_none() {
                index =
                    Some(self.repo.index_or_empty().map_err(|err| {
                        GitError::Internal(format!("cannot read the index: {err}"))
                    })?);
            }
            let name = path.trim_end_matches('/');
            let tracked = index.as_ref().is_some_and(|index| {
                index.entry_by_path(name.into()).is_some()
                    || index
                        .prefixed_entries(format!("{name}/").as_str().into())
                        .is_some()
            });
            if tracked {
                kept.push(path.clone());
            }
        }
        Ok(kept)
    }

    /// Read by `gix`, which costs no process: the common case has none.
    fn unmerged_paths(&self) -> Result<Vec<String>> {
        let index = self.current_index()?;
        let mut paths: Vec<String> = index
            .entries()
            .iter()
            .filter(|entry| entry.stage_raw() != 0)
            .map(|entry| entry.path(&index).to_string())
            .collect();
        paths.dedup();
        Ok(paths)
    }

    /// Those of `paths` whose staged side `stash` recorded: its index commit (`^2`) differs
    /// from HEAD at the time (`^1`) there.
    pub fn staged_in_stash(&self, stash: &str, paths: &[String]) -> Result<Vec<String>> {
        let commit = self.find_commit(stash)?;
        let parents: Vec<gix::ObjectId> = commit.parent_ids().map(|id| id.detach()).collect();
        let (Some(&base), Some(&index)) = (parents.first(), parents.get(1)) else {
            return Err(GitError::InvalidState(format!("{stash} is not a stash")));
        };
        let (base, index) = (self.tree_of(base)?, self.tree_of(index)?);
        let entry = |tree: &gix::Tree<'_>, path: &str| {
            tree.lookup_entry_by_path(path)
                .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))
                .map(|entry| entry.map(|entry| (entry.mode(), entry.object_id())))
        };
        let mut staged = Vec::new();
        for path in paths.iter().filter(|path| !path.ends_with('/')) {
            if entry(&base, path)? != entry(&index, path)? {
                staged.push(path.clone());
            }
        }
        Ok(staged)
    }

    /// Puts the staged side `stash` recorded for `paths` back into the index and the
    /// working tree, a deletion included.
    pub fn restore_staged_from(&self, stash: &str, paths: &[String]) -> Result<()> {
        let source = format!("--source={stash}^2");
        self.run_git_paths(&["restore", &source, "--staged", "--worktree"], paths)
            .map(drop)
    }

    /// A stash that leaves the working tree as it is: `stash create` builds the commit
    /// without touching a file, `stash store` lists it. Untracked files are not in it —
    /// `stash create` has no `--include-untracked` (R-212).
    pub fn stash_keeping_worktree(&self, message: &str) -> Result<()> {
        let created = self.run_git(&["stash", "create", message])?;
        let oid = created.stdout.trim();
        if oid.is_empty() {
            return Err(GitError::InvalidState(
                "there is nothing to stash".to_owned(),
            ));
        }
        self.run_git(&["stash", "store", "--message", message, oid])
            .map(drop)
    }

    /// Without `--index` only after a refusal that changed nothing: an attempt that stopped
    /// on a conflict has written its markers, and a retry could only report "needs merge"
    /// in place of git's account of the conflict (INV-05).
    pub fn stash_apply(&self, oid: &str) -> Result<()> {
        let Err(first) = self.run_git(&["stash", "apply", "--index", oid]) else {
            return Ok(());
        };
        let conflicted = self.conflicted_paths().map_or_else(
            |err| {
                tracing::error!(error = ?err, context = "cannot tell whether stash apply left a conflict");
                true
            },
            |paths| !paths.is_empty(),
        );
        if conflicted {
            return Err(first);
        }
        self.run_git(&["stash", "apply", oid]).map(drop)
    }

    /// Whether `--include-untracked` has anything to take. Without one it only makes `stash`
    /// start two more processes of its own and record an empty third parent (R-315).
    fn has_untracked(&self) -> Result<bool> {
        let iter = self
            .status_platform()?
            .untracked_files(gix::status::UntrackedFiles::Collapsed)
            .into_index_worktree_iter(Vec::<gix::bstr::BString>::new())
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;
        for item in iter {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            if let gix::status::index_worktree::Item::DirectoryContents { entry, .. } = item
                && entry.status == gix::dir::entry::Status::Untracked
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Through `gix`: a spawned `rev-parse` costs tens of milliseconds (R-24).
    fn stash_top(&self) -> Option<String> {
        self.repo
            .find_reference("refs/stash")
            .ok()?
            .peel_to_id()
            .ok()
            .map(|id| id.detach().to_string())
    }
}

impl RepoHandle {
    /// Newest first, matching `stash@{0}`. The stash reflog is the list (R-196).
    pub fn stashes(&self) -> Result<Vec<StashEntry>> {
        Ok(self
            .reflog_of("refs/stash", usize::MAX)?
            .into_iter()
            .map(|line| StashEntry {
                index: u32::try_from(line.position).unwrap_or(u32::MAX),
                oid: line.oid.to_string(),
                message: line.message,
                timestamp: line.author_time,
            })
            .collect())
    }

    pub fn stash_push(&self, options: &StashOptions) -> Result<()> {
        match self.stash_push_if_any(options)? {
            Some(_) => Ok(()),
            None => Err(GitError::InvalidState(
                "there is nothing to stash".to_owned(),
            )),
        }
    }

    /// The new stash's oid, or `None` when git found nothing it stashes — a moved
    /// submodule shows as a change and still leaves nothing to save.
    pub fn stash_push_if_any(&self, options: &StashOptions) -> Result<Option<String>> {
        let mut args = vec!["stash", "push"];
        if options.include_untracked && self.has_untracked()? {
            args.push("--include-untracked");
        }
        if options.keep_index {
            args.push("--keep-index");
        }
        if !options.message.trim().is_empty() {
            args.push("--message");
            args.push(&options.message);
        }

        let before = self.stash_top();
        self.run_git(&args)?;
        let after = self.stash_top();
        Ok(if after == before { None } else { after })
    }

    pub fn stash_apply_index(&self, index: u32, pop: bool) -> Result<()> {
        let reference = self.stash_ref(index)?;
        let verb = if pop { "pop" } else { "apply" };
        self.run_git(&["stash", verb, &reference]).map(drop)
    }

    /// `stash pop` of the entry that is `oid`, looked up as it runs: a stash made or dropped
    /// since this one was would have moved it off `stash@{0}`.
    pub fn stash_pop_oid(&self, oid: &str) -> Result<()> {
        let entry = self
            .stashes()?
            .into_iter()
            .find(|entry| entry.oid == oid)
            .ok_or_else(|| {
                GitError::InvalidState(format!(
                    "the stash {} is no longer in the list",
                    oid.get(..7).unwrap_or(oid)
                ))
            })?;
        let reference = format!("stash@{{{}}}", entry.index);
        self.run_git(&["stash", "pop", &reference]).map(drop)
    }

    /// Stash, switch, put the changes back — what `--autostash` does for rebase and pull —
    /// in one call, so the lane runs it as one operation: between separate steps another
    /// stash operation could shift `stash@{0}` (R-521). A refused switch puts the changes
    /// back and returns the refusal; a pop that conflicts after the switch returns git's
    /// account of the conflict, and git keeps the stash.
    pub fn switch_with_autostash(
        &self,
        target: &crate::CheckoutTarget,
        message: &str,
    ) -> Result<()> {
        let made = self.stash_push_if_any(&StashOptions {
            message: message.to_owned(),
            include_untracked: true,
            keep_index: false,
        })?;
        let switched = self.checkout(target);
        let Some(oid) = made else {
            return switched;
        };
        let restored = self.stash_pop_oid(&oid);
        match (switched, restored) {
            (Err(refused), Err(err)) => {
                // A failed pop is a git command of its own and reaches the journal; the
                // refusal is what the caller asked about.
                tracing::error!(error = ?err, stash = %oid, context = "autostash: the changes stay in the stash after a refused switch");
                Err(refused)
            }
            (Err(refused), Ok(())) => Err(refused),
            (Ok(()), restored) => restored,
        }
    }

    /// Returns the dropped entry so Undo can put it back (`restore_stash`).
    pub fn stash_drop(&self, index: u32) -> Result<StashEntry> {
        let entry = self
            .stashes()?
            .into_iter()
            .find(|entry| entry.index == index)
            .ok_or_else(|| GitError::InvalidState(format!("no stash at index {index}")))?;
        self.run_git(&["stash", "drop", &self.stash_ref(index)?])?;
        Ok(entry)
    }

    fn stash_ref(&self, index: u32) -> Result<String> {
        if !self.stashes()?.iter().any(|entry| entry.index == index) {
            return Err(GitError::InvalidState(format!("no stash at index {index}")));
        }
        Ok(format!("stash@{{{index}}}"))
    }
    /// Reads a stash through `gix`: applying it to look at it would be the one thing the
    /// user did not ask for (T5.2).
    pub fn stash_contents(&self, index: u32) -> Result<StashContents> {
        // `stash@{n}` rather than `stash list`: the reflog selector resolves inside gix, so
        // looking at a stash costs no process at all.
        let selector = format!("stash@{{{index}}}");
        let id = self
            .repo
            .rev_parse_single(selector.as_str())
            .map_err(|_| GitError::InvalidState(format!("no {selector}")))?;
        let stash = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::Internal(format!("cannot read {selector}: {err}")))?;

        let parents: Vec<gix::ObjectId> = stash.parent_ids().map(|id| id.detach()).collect();
        let base = *parents
            .first()
            .ok_or_else(|| GitError::InvalidState("a stash without a parent".to_owned()))?;
        let index_rev = *parents.get(1).unwrap_or(&base);
        let untracked_rev = parents.get(2).copied();

        let base_tree = self.tree_of(base)?;
        let worktree = self.files_between_trees(
            &base_tree,
            &self.tree_of(stash.id().detach())?,
            crate::DEFAULT_SIMILARITY,
        )?;
        let staged = self.files_between_trees(
            &base_tree,
            &self.tree_of(index_rev)?,
            crate::DEFAULT_SIMILARITY,
        )?;
        let untracked = match untracked_rev {
            // Against the empty tree: every file in that commit is new by construction.
            Some(oid) => self.files_between_trees(
                &self.repo.empty_tree(),
                &self.tree_of(oid)?,
                crate::DEFAULT_SIMILARITY,
            )?,
            None => Vec::new(),
        };

        Ok(StashContents {
            worktree,
            index: staged,
            untracked,
            base: base.to_string(),
            worktree_rev: stash.id().to_string(),
            index_rev: index_rev.to_string(),
            untracked_rev: untracked_rev.map(|oid| oid.to_string()),
        })
    }
}
