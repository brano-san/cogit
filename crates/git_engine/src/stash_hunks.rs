use crate::{GitError, Head, PatchTarget, RepoHandle, Result};

const APPLY: [&str; 3] = ["apply", "--unidiff-zero", "--whitespace=nowarn"];

fn explained(err: GitError, summary: &str) -> GitError {
    match err {
        GitError::Command(mut failed) => {
            summary.clone_into(&mut failed.summary);
            GitError::Command(failed)
        }
        other => other,
    }
}

impl RepoHandle {
    pub fn stash_hunks(&self, stage: &str, discard: &str, message: &str) -> Result<String> {
        if stage.trim().is_empty() || discard.trim().is_empty() {
            return Err(GitError::InvalidState("no lines selected".to_owned()));
        }
        let (branch, head) = match self.head()? {
            Head::Branch { name, oid } => (name, oid),
            Head::Detached { oid } => ("(no branch)".to_owned(), oid),
            Head::Unborn { .. } => {
                return Err(GitError::InvalidState(
                    "Nothing is committed yet: there is no HEAD to stash lines against".to_owned(),
                ));
            }
        };
        let scratch = crate::commit_write::Scratch::beside_index(self, "stash-hunks");
        self.run_git_indexed(&scratch.0, &["read-tree", &head], None)?;
        let onto_head = [&["apply", "--cached"][..], &APPLY[1..], &["-"]].concat();
        self.run_git_indexed(&scratch.0, &onto_head, Some(stage.as_bytes()))
            .map_err(|err| {
                explained(
                    err,
                    "The selected lines build on staged changes: stage or unstage them first, or stash the whole file",
                )
            })?;
        let check = [&APPLY[..], &["--check", "--reverse", "-"]].concat();
        self.run_git_fed(&check, discard.as_bytes())?;
        let tree = self.run_git_indexed(&scratch.0, &["write-tree"], None)?;
        let tree = tree.stdout.trim();

        let commit = self.find_commit(&head)?;
        let base_tree = commit
            .tree_id()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of {head}: {err}")))?
            .to_string();
        if base_tree == tree {
            return Err(GitError::InvalidState(
                "the selected lines change nothing against HEAD".to_owned(),
            ));
        }
        let subject = commit
            .message()
            .map_err(|err| GitError::Internal(format!("cannot read {head}: {err}")))?
            .summary()
            .to_string();
        let short = commit.id().shorten_or_id().to_string();
        let index = self.commit_tree(
            &base_tree,
            &[&head],
            &format!("index on {branch}: {short} {subject}"),
        )?;
        let message = if message.trim().is_empty() {
            format!("WIP on {branch}: {short} {subject}")
        } else {
            format!("On {branch}: {message}")
        };
        let stash = self.commit_tree(tree, &[&head, &index], &message)?;
        self.run_git(&["stash", "store", "--message", &message, &stash])?;
        if let Err(err) = self.apply_patch_to(discard, true, PatchTarget::WorkTree) {
            self.take_off_the_list(&stash);
            return Err(err);
        }
        Ok(stash)
    }

    pub fn unstash_hunks(&self, stash: &str, discard: &str) -> Result<()> {
        let entry = self.stash_by_oid(stash).map_err(|_| {
            GitError::InvalidState(
                "the stash of those lines is no longer listed: applied or dropped since".to_owned(),
            )
        })?;
        self.apply_patch_to(discard, false, PatchTarget::WorkTree)?;
        self.stash_drop(&entry.oid).map(drop)
    }

    fn take_off_the_list(&self, stash: &str) {
        if self.stash_top().as_deref() != Some(stash) {
            return;
        }
        if let Err(err) = self.run_git(&["stash", "drop", "--quiet", "stash@{0}"]) {
            tracing::error!(error = ?err, stash, context = "stash of lines left listed after the working tree refused them");
        }
    }
}
