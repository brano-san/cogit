use crate::{GitError, Head, RepoHandle, Result};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplitCut {
    pub patch: String,
    pub paths: Vec<String>,
}

#[derive(Debug)]
pub struct SplitMove {
    pub branch: String,
    pub from: String,
    pub to: String,
}

struct SplitHistory {
    branch: String,
    head: String,
    target: gix::ObjectId,
    parent: Option<gix::ObjectId>,
    descendants: Vec<String>,
}

impl RepoHandle {
    pub fn commit_id(&self, rev: &str) -> Result<String> {
        Ok(self.resolve_commit(rev)?.to_string())
    }

    pub fn split_commit_blocked(&self, rev: &str) -> Result<Option<String>> {
        match self.checked_split_history(rev) {
            Ok(_) => Ok(None),
            Err(GitError::InvalidState(reason)) => Ok(Some(reason)),
            Err(err) => Err(err),
        }
    }

    pub fn split_commit(
        &self,
        rev: &str,
        cuts: &[SplitCut],
        messages: &[String],
    ) -> Result<SplitMove> {
        if messages.len() < 2 || messages.len() != cuts.len() {
            return Err(GitError::InvalidState(
                "a split needs at least two commits, each with its blocks".to_owned(),
            ));
        }
        if let Some(blank) = messages.iter().position(|m| m.trim().is_empty()) {
            return Err(GitError::InvalidState(format!(
                "commit {} of the split needs a message",
                blank + 1
            )));
        }
        let history = self.checked_split_history(rev)?;
        let target = history.target.to_string();
        if !history.descendants.is_empty() {
            self.require_clean_for_rebase()?;
        }
        let trees = self.split_trees(&history, cuts)?;
        let mut tip = history.parent.map(|id| id.to_string());
        let commit = self.find_commit(&target)?;
        let author = commit
            .author()
            .map_err(|err| GitError::Internal(format!("cannot read commit author: {err}")))?;
        let identity = |bytes| {
            std::str::from_utf8(bytes).map_err(|_| {
                GitError::InvalidState("the original author identity is not valid UTF-8".to_owned())
            })
        };
        let time = author
            .time()
            .map_err(|err| GitError::Internal(format!("cannot read author date: {err}")))?;
        let minutes = time.offset.unsigned_abs() / 60;
        let sign = if time.offset < 0 { '-' } else { '+' };
        let date = format!(
            "@{} {sign}{:02}{:02}",
            time.seconds,
            minutes / 60,
            minutes % 60
        );
        let env = [
            ("GIT_AUTHOR_NAME", identity(author.name.as_ref())?),
            ("GIT_AUTHOR_EMAIL", identity(author.email.as_ref())?),
            ("GIT_AUTHOR_DATE", date.as_str()),
        ];
        let signing = if self
            .repo
            .config_snapshot()
            .boolean("commit.gpgSign")
            .unwrap_or(false)
        {
            "-S"
        } else {
            "--no-gpg-sign"
        };
        for (tree, message) in trees.iter().zip(messages) {
            let tree = tree.to_string();
            let mut args = vec![
                "-c",
                "i18n.commitEncoding=UTF-8",
                "commit-tree",
                &tree,
                signing,
            ];
            if let Some(parent) = tip.as_deref() {
                args.extend(["-p", parent]);
            }
            args.extend(["-F", "-"]);
            let mut text = message.replace("\r\n", "\n");
            if !text.ends_with('\n') {
                text.push('\n');
            }
            tip = Some(
                self.run_git_fed_with_env(&args, &env, text.as_bytes())?
                    .stdout
                    .trim()
                    .to_owned(),
            );
        }

        let head = Head::Branch {
            name: history.branch.clone(),
            oid: history.head.clone(),
        };
        if self.head()? != head || self.state()?.is_interrupted_operation() {
            return Err(GitError::InvalidState(
                "the checked-out branch or its operation changed during the split".to_owned(),
            ));
        }
        let tip =
            tip.ok_or_else(|| GitError::Internal("no split commits were written".to_owned()))?;
        let new_tip = if history.descendants.is_empty() {
            let reason = format!("cogit: split {}", target.get(..7).unwrap_or(&target));
            let update = format!(
                "update refs/heads/{} {tip} {}\nupdate ORIG_HEAD {}\n",
                history.branch, history.head, history.head
            );
            self.run_git_fed(&["update-ref", "-m", &reason, "--stdin"], update.as_bytes())?;
            tip
        } else {
            self.replay_split(&target, &tip, &history.descendants)?;
            self.branch_tip(&history.branch)?
        };
        Ok(SplitMove {
            branch: history.branch,
            from: history.head,
            to: new_tip,
        })
    }

    fn checked_split_history(&self, rev: &str) -> Result<SplitHistory> {
        let history = self.split_history(rev)?;
        let target = history.target.to_string();
        let protecting = self.protecting_refs(&target)?;
        if !protecting.is_empty() {
            return Err(GitError::InvalidState(format!(
                "{} is on {} — splitting it would rewrite what others already have",
                target.get(..7).unwrap_or(&target),
                protecting.join(", ")
            )));
        }
        Ok(history)
    }

    fn split_history(&self, rev: &str) -> Result<SplitHistory> {
        let state = self.state()?;
        if state.is_interrupted_operation() {
            return Err(GitError::InvalidState(
                "finish or abort the operation in progress first".to_owned(),
            ));
        }
        let Head::Branch {
            name: branch,
            oid: head,
        } = self.head()?
        else {
            return Err(GitError::InvalidState("HEAD is not on a branch".to_owned()));
        };
        if state == crate::RepoState::Bare {
            return Err(GitError::InvalidState("the repository is bare".to_owned()));
        }
        let target = self.resolve_commit(rev)?;
        let target_commit = self.split_object(target)?;
        let mut parents = target_commit.parent_ids();
        let parent = parents.next().map(|id| id.detach());
        if parents.next().is_some() {
            return Err(GitError::InvalidState(
                "the selected commit is a merge commit".to_owned(),
            ));
        }

        let mut current = self.resolve_commit(&head)?;
        let mut descendants = Vec::new();
        let mut replay_has_merge = false;
        while current != target {
            let commit = self.split_object(current)?;
            let mut parents = commit.parent_ids();
            let Some(first) = parents.next() else {
                return Err(GitError::InvalidState(
                    "the commit is not on the first-parent history of the checked-out branch"
                        .to_owned(),
                ));
            };
            replay_has_merge |= parents.next().is_some();
            descendants.push(current.to_string());
            current = first.detach();
        }
        if replay_has_merge {
            return Err(GitError::InvalidState(
                "a merge commit comes after this one: rewriting under it would flatten the merge"
                    .to_owned(),
            ));
        }
        descendants.reverse();
        Ok(SplitHistory {
            branch,
            head,
            target,
            parent,
            descendants,
        })
    }

    fn split_object(&self, oid: gix::ObjectId) -> Result<gix::Commit<'_>> {
        self.repo
            .find_commit(oid)
            .map_err(|err| GitError::Internal(format!("cannot read {oid}: {err}")))
    }

    fn split_trees(&self, history: &SplitHistory, cuts: &[SplitCut]) -> Result<Vec<gix::ObjectId>> {
        let target = self.tree_of(history.target)?;
        let base = match history.parent {
            Some(parent) => self.tree_of(parent)?,
            None => self.repo.empty_tree(),
        };
        let mut trees = Vec::with_capacity(cuts.len());
        let mut previous = base.id().detach();
        for (at, cut) in cuts.iter().enumerate() {
            let scratch = crate::commit_write::Scratch::beside_index(self, "split-commit");
            if let Some(parent) = history.parent {
                self.run_git_indexed(&scratch.0, &["read-tree", &parent.to_string()], None)?;
            } else {
                self.run_git_indexed(&scratch.0, &["read-tree", "--empty"], None)?;
            }
            if !cut.paths.is_empty() {
                let null = self.repo.object_hash().null();
                let mut entries = String::new();
                let mut kept = String::new();
                let mut removed = HashSet::new();
                let mut written_paths = Vec::new();
                for path in &cut.paths {
                    match target
                        .lookup_entry_by_path(path.as_str())
                        .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?
                        .filter(|entry| !entry.mode().is_tree())
                    {
                        Some(entry) => {
                            written_paths.push(path.as_str());
                            kept.push_str(&format!(
                                "{} {}\t{path}\0",
                                entry.mode().kind().as_octal_str(),
                                entry.object_id()
                            ));
                        }
                        None => {
                            removed.insert(path.as_str());
                            entries.push_str(&format!("0 {null}\t{path}\0"));
                        }
                    }
                }
                for path in written_paths {
                    self.check_split_path(&base, path, &removed)?;
                }
                entries.push_str(&kept);
                self.run_git_indexed(
                    &scratch.0,
                    &["update-index", "-z", "--index-info"],
                    Some(entries.as_bytes()),
                )?;
            }
            if !cut.patch.is_empty() {
                self.run_git_indexed(
                    &scratch.0,
                    &[
                        "apply",
                        "--cached",
                        "--unidiff-zero",
                        "--whitespace=nowarn",
                        "-",
                    ],
                    Some(cut.patch.as_bytes()),
                )?;
            }
            let written = self.run_git_indexed(&scratch.0, &["write-tree"], None)?;
            let tree =
                gix::ObjectId::from_hex(written.stdout.trim().as_bytes()).map_err(|err| {
                    GitError::Internal(format!("cannot read the split tree ID: {err}"))
                })?;
            if tree == previous {
                return Err(GitError::InvalidState(format!(
                    "commit {} of the split changes nothing: give it a block",
                    at + 1
                )));
            }
            previous = tree;
            trees.push(tree);
        }
        if previous != target.id().detach() {
            return Err(GitError::InvalidState(
                "the split does not reproduce the original commit tree".to_owned(),
            ));
        }
        Ok(trees)
    }

    fn check_split_path(
        &self,
        base: &gix::Tree<'_>,
        path: &str,
        removed: &HashSet<&str>,
    ) -> Result<()> {
        let blocked = |prior: &str| {
            GitError::InvalidState(format!(
                "Remove {prior} before or in the same part as adding {path}"
            ))
        };
        let read_error = |err| GitError::Internal(format!("cannot look up {path}: {err}"));
        for (at, _) in path.match_indices('/') {
            let prefix = &path[..at];
            if !removed.contains(prefix)
                && base
                    .lookup_entry_by_path(prefix)
                    .map_err(read_error)?
                    .is_some_and(|entry| !entry.mode().is_tree())
            {
                return Err(blocked(prefix));
            }
        }
        if let Some(entry) = base
            .lookup_entry_by_path(path)
            .map_err(read_error)?
            .filter(|entry| entry.mode().is_tree())
        {
            let directory = self.repo.find_tree(entry.object_id()).map_err(|err| {
                GitError::Internal(format!("cannot read directory {path}: {err}"))
            })?;
            let mut recorder = gix::traverse::tree::Recorder::default();
            directory
                .traverse()
                .breadthfirst(&mut recorder)
                .map_err(|err| {
                    GitError::Internal(format!("cannot walk directory {path}: {err}"))
                })?;
            for child in recorder
                .records
                .into_iter()
                .filter(|entry| !entry.mode.is_tree())
            {
                let prior = format!("{path}/{}", child.filepath);
                if !removed.contains(prior.as_str()) {
                    return Err(blocked(&prior));
                }
            }
        }
        Ok(())
    }
}
