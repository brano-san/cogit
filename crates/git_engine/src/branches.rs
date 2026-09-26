use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

/// What deleting a branch on the server came to (R-480).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteDeletion {
    Deleted,
    /// The server no longer had it; only the stale remote-tracking ref went.
    AlreadyGone,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CheckoutTarget {
    Branch {
        name: String,
    },
    Commit {
        oid: String,
    },
    /// A local branch made at `start` and checked out; `track` makes `start`, a remote
    /// branch, its upstream.
    NewBranch {
        name: String,
        start: String,
        track: bool,
    },
    /// An existing local branch moved forward to `to` and checked out; never back or
    /// sideways.
    FastForward {
        name: String,
        to: String,
    },
}

impl CheckoutTarget {
    /// What the safety journal says was checked out.
    #[must_use]
    pub fn described(&self) -> String {
        match self {
            Self::Branch { name } => name.clone(),
            Self::Commit { oid } => oid.clone(),
            Self::NewBranch { name, start, .. } => format!("new branch {name} at {start}"),
            Self::FastForward { name, to } => format!("{name}, fast-forwarded to {to}"),
        }
    }
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
            CheckoutTarget::NewBranch { name, start, track } => {
                let name = require_name(name)?;
                let start = require_name(start)?;
                // Explicit either way: `branch.autoSetupMerge` tracks a remote start unasked.
                let track = if *track { "--track" } else { "--no-track" };
                self.run_git(&["switch", track, "--create", name, start])
                    .map(drop)
            }
            CheckoutTarget::FastForward { name, to } => self.fast_forward_to(name, to),
        }
    }

    /// `switch -C` resets the branch only after the working tree has switched, so a
    /// refusal moves nothing. It is given the commit, not the remote ref: a ref as the
    /// start point would set the upstream again.
    fn fast_forward_to(&self, name: &str, to: &str) -> Result<()> {
        let name = require_name(name)?;
        let to = self.resolve_commit(require_name(to)?)?.to_string();
        let branch = format!("refs/heads/{name}");
        if !self.is_ancestor(&branch, &to)? {
            return Err(GitError::InvalidState(format!(
                "{name} has commits that {} does not; it cannot fast-forward",
                to.get(..7).unwrap_or(&to)
            )));
        }
        self.run_git(&["switch", "-C", name, &to]).map(drop)
    }

    /// Puts an existing branch back at `oid`. The checked-out one goes through
    /// `reset --keep`, which refuses rather than overwrites local changes; any other is
    /// moved with `branch --force`.
    pub fn move_branch_back(&self, name: &str, oid: &str) -> Result<()> {
        let name = require_name(name)?;
        match self.head()? {
            crate::Head::Branch { name: current, .. } if current == name => {
                self.reset(oid, crate::ResetMode::Keep)
            }
            _ => self.run_git(&["branch", "--force", name, oid]).map(drop),
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

    /// `git branch -m` moves HEAD with the branch, so a rename of the checked-out branch
    /// needs no extra step.
    pub fn rename_branch(&self, from: &str, to: &str, force: bool) -> Result<()> {
        let from = require_name(from)?;
        let to = require_name(to)?;
        let flag = if force { "-M" } else { "-m" };
        self.run_git(&["branch", flag, from, to]).map(drop)
    }

    /// `None` clears the tracking information rather than pointing it somewhere harmless.
    pub fn set_upstream(&self, branch: &str, upstream: Option<&str>) -> Result<()> {
        let branch = require_name(branch)?;
        match upstream {
            Some(upstream) => {
                let upstream = require_name(upstream)?;
                self.run_git(&["branch", "--set-upstream-to", upstream, branch])
                    .map(drop)
            }
            None => self
                .run_git(&["branch", "--unset-upstream", branch])
                .map(drop),
        }
    }

    /// `push --delete` rather than deleting the tracking ref: only the push reaches the
    /// server's hooks and permissions, and a local ref deletion would quietly lie. The
    /// server is asked first: a branch already gone is done, not a failure (R-480).
    pub fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<RemoteDeletion> {
        let remote = require_name(remote)?;
        let branch = require_name(branch)?;
        // The panel shows `origin/topic`; the tracking ref under it is the one to map.
        let short = branch.strip_prefix(&format!("{remote}/")).unwrap_or(branch);
        let tracking = format!("refs/remotes/{remote}/{short}");
        // In full: beside a tag of the same name, `topic` alone matches more than one.
        let on_server = self
            .tracked_name(remote, &tracking)
            .unwrap_or_else(|| format!("refs/heads/{short}"));
        let listed = self.read_git(&["ls-remote", remote, &on_server])?;
        let there = listed.lines().any(|line| {
            line.split_once('\t')
                .is_some_and(|(_, name)| name == on_server)
        });
        if there {
            self.run_git(&["push", "--delete", remote, &on_server])?;
            return Ok(RemoteDeletion::Deleted);
        }
        if self.repo.find_reference(tracking.as_str()).is_ok() {
            self.run_git(&["update-ref", "-d", &tracking])?;
        }
        Ok(RemoteDeletion::AlreadyGone)
    }

    /// The server's name for what `tracking` follows, by `remote`'s fetch refspecs.
    fn tracked_name(&self, remote: &str, tracking: &str) -> Option<String> {
        let name = gix::refs::FullName::try_from(tracking).ok()?;
        let (upstream, found) = self
            .repo
            .upstream_branch_and_remote_for_tracking_branch(name.as_ref())
            .ok()??;
        let same = found.name().is_some_and(|name| name.as_bstr() == remote);
        same.then(|| upstream.as_bstr().to_string())
    }
}

fn require_name(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(GitError::InvalidState("an empty name".to_owned()));
    }
    Ok(trimmed)
}
