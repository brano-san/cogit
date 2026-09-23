//! Remote ▸ Subtree (#45), over `git subtree` — the script Git for Windows ships.

use crate::{GitError, RepoHandle, Result};
use std::collections::BTreeSet;

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SubtreeOp {
    Add {
        prefix: String,
        repository: String,
        reference: String,
        squash: bool,
    },
    /// `git subtree pull` from `repository`, or `git subtree merge` of a local commit.
    Merge {
        prefix: String,
        repository: Option<String>,
        reference: String,
        squash: bool,
    },
    /// The folder's own history as a new branch.
    Split {
        prefix: String,
        branch: String,
        rejoin: bool,
    },
    /// The folder's contents replaced by `reference`'s tree, staged for the next commit.
    Reset { prefix: String, reference: String },
    Push {
        prefix: String,
        repository: String,
        reference: String,
    },
}

/// Commits read for `git-subtree-dir:`; an older subtree is typed in by hand.
const PREFIX_SCAN_LIMIT: usize = 50_000;

impl RepoHandle {
    pub fn subtree_op(&self, op: &SubtreeOp) -> Result<()> {
        match op {
            SubtreeOp::Add {
                prefix,
                repository,
                reference,
                squash,
            } => {
                let at = prefix_arg(prefix)?;
                let mut args = vec!["subtree", "add", at.as_str()];
                if *squash {
                    args.push("--squash");
                }
                args.extend([
                    required(repository, "repository")?,
                    required(reference, "ref")?,
                ]);
                self.run_git(&args).map(drop)
            }
            SubtreeOp::Merge {
                prefix,
                repository,
                reference,
                squash,
            } => {
                let at = prefix_arg(prefix)?;
                let source = repository
                    .as_deref()
                    .map(str::trim)
                    .filter(|r| !r.is_empty());
                let mut args = vec!["subtree", if source.is_some() { "pull" } else { "merge" }];
                args.push(at.as_str());
                if *squash {
                    args.push("--squash");
                }
                args.extend(source);
                args.push(required(reference, "ref")?);
                self.run_git(&args).map(drop)
            }
            SubtreeOp::Split {
                prefix,
                branch,
                rejoin,
            } => {
                let at = prefix_arg(prefix)?;
                let mut args = vec![
                    "subtree",
                    "split",
                    at.as_str(),
                    "-b",
                    required(branch, "branch")?,
                ];
                if *rejoin {
                    args.push("--rejoin");
                }
                self.run_git(&args).map(drop)
            }
            SubtreeOp::Reset { prefix, reference } => {
                self.reset_subtree(&clean_prefix(prefix)?, required(reference, "ref")?)
            }
            SubtreeOp::Push {
                prefix,
                repository,
                reference,
            } => {
                let at = prefix_arg(prefix)?;
                let args = [
                    "subtree",
                    "push",
                    at.as_str(),
                    required(repository, "repository")?,
                    required(reference, "ref")?,
                ];
                self.run_git(&args).map(drop)
            }
        }
    }

    /// Folders `git subtree add` recorded in the history and still present.
    pub fn subtree_prefixes(&self) -> Result<Vec<String>> {
        let Ok(head) = self.repo.head_id() else {
            return Ok(Vec::new());
        };
        let walk = self
            .repo
            .rev_walk(Some(head.detach()))
            .all()
            .map_err(|err| GitError::Internal(format!("cannot walk history: {err}")))?;
        let mut found = BTreeSet::new();
        for info in walk
            .filter_map(std::result::Result::ok)
            .take(PREFIX_SCAN_LIMIT)
        {
            let Ok(commit) = self.repo.find_commit(info.id) else {
                continue;
            };
            let Ok(message) = commit.message_raw() else {
                continue;
            };
            for line in message.to_string().lines() {
                if let Some(dir) = line.strip_prefix("git-subtree-dir:") {
                    found.insert(dir.trim().trim_end_matches('/').to_owned());
                }
            }
        }
        Ok(found
            .into_iter()
            .filter(|dir| !dir.is_empty() && self.root().join(dir).is_dir())
            .collect())
    }

    /// Refuses a folder with uncommitted work: the reset would take it away unasked.
    fn reset_subtree(&self, prefix: &str, reference: &str) -> Result<()> {
        let tree = format!("{reference}^{{tree}}");
        self.run_git_reading(&["rev-parse", "--verify", "--quiet", &tree])?;
        let dirty = self.run_git_reading(&[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            prefix,
        ])?;
        if !dirty.stdout.trim().is_empty() {
            return Err(GitError::InvalidState(format!(
                "{prefix}/ has uncommitted changes; commit or stash them first"
            )));
        }
        self.run_git(&["rm", "-r", "-q", "--ignore-unmatch", "--", prefix])?;
        let at = format!("--prefix={prefix}/");
        self.run_git(&["read-tree", &at, "-u", reference]).map(drop)
    }
}

fn required<'a>(value: &'a str, what: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(GitError::InvalidState(format!("enter a {what}")));
    }
    Ok(value)
}

fn clean_prefix(prefix: &str) -> Result<String> {
    let prefix = prefix.trim().replace('\\', "/");
    let prefix = prefix.trim_end_matches('/');
    let escapes = prefix.starts_with('/')
        || prefix.contains(':')
        || prefix
            .split('/')
            .any(|part| part == ".." || part.is_empty());
    if prefix.is_empty() || escapes {
        return Err(GitError::InvalidState(format!(
            "\"{prefix}\" is not a folder inside the repository"
        )));
    }
    Ok(prefix.to_owned())
}

fn prefix_arg(prefix: &str) -> Result<String> {
    Ok(format!("--prefix={}", clean_prefix(prefix)?))
}
