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

impl RepoHandle {
    pub fn stash_paths(&self, paths: &[String], message: &str) -> Result<Option<String>> {
        // Git hands the list on to a `git clean` of its own, on a command line: past its
        // limit the stash is made and the files stay (R-191).
        if !crate::runner::fits_command_line(paths) {
            return Err(GitError::InvalidState(format!(
                "{} paths are too many to stash at once",
                paths.len()
            )));
        }
        let before = self.stash_top();

        let mut args = vec!["stash", "push", "--include-untracked"];
        if !message.trim().is_empty() {
            args.extend(["--message", message]);
        }
        self.run_git_paths(&args, paths)?;

        let after = self.stash_top();
        Ok(if after == before { None } else { after })
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

    pub fn stash_apply(&self, oid: &str) -> Result<()> {
        self.run_git(&["stash", "apply", "--index", oid])
            .or_else(|_| self.run_git(&["stash", "apply", oid]))
            .map(drop)
    }

    /// Whether `--include-untracked` has anything to take. Without one it only makes `stash`
    /// start two more processes of its own and record an empty third parent (R-315).
    fn has_untracked(&self) -> Result<bool> {
        let iter = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
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

    /// Returns the dropped commit so Undo can put the entry back.
    pub fn stash_drop(&self, index: u32) -> Result<String> {
        let oid = self
            .stashes()?
            .into_iter()
            .find(|entry| entry.index == index)
            .ok_or_else(|| GitError::InvalidState(format!("no stash at index {index}")))?
            .oid;
        self.run_git(&["stash", "drop", &self.stash_ref(index)?])?;
        Ok(oid)
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
