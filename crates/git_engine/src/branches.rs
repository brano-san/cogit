use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CheckoutTarget {
    Branch { name: String },
    Commit { oid: String },
}

impl RepoHandle {
    pub fn checkout(&self, target: &CheckoutTarget) -> Result<()> {
        match target {
            CheckoutTarget::Branch { name } => {
                let name = require_name(name)?;
                self.run_git(&["switch", "--", name]).map(drop)
            }
            // `--detach` is explicit: `switch` refuses a raw commit without it, and the
            // user asking for a commit is asking for a detached HEAD.
            CheckoutTarget::Commit { oid } => {
                let oid = require_name(oid)?;
                self.run_git(&["switch", "--detach", oid]).map(drop)
            }
        }
    }

    pub fn create_branch(&self, name: &str, start: Option<&str>, switch: bool) -> Result<()> {
        let name = require_name(name)?;
        let mut args = vec![if switch { "switch" } else { "branch" }];
        if switch {
            args.push("--create");
        }
        args.push(name);
        if let Some(start) = start {
            args.push(start);
        }
        self.run_git(&args).map(drop)
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let name = require_name(name)?;
        let flag = if force { "-D" } else { "-d" };
        self.run_git(&["branch", flag, "--", name]).map(drop)
    }
}

fn require_name(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(GitError::InvalidState("an empty name".to_owned()));
    }
    Ok(trimmed)
}
