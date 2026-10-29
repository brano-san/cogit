//! The light status of a row of the Repositories list that is not on screen (R-353).

use crate::{RepoHandle, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoPulse {
    pub missing: bool,
    pub branch: Option<String>,
    /// On a detached HEAD, a branch whose tip it is, else `abc1234: subject` (`detached_name`).
    pub branch_at: Option<String>,
    /// HEAD's branch has an upstream and its remote-tracking ref exists locally.
    pub tracked: bool,
    pub ahead: u32,
    pub behind: u32,
    /// `!status().is_clean()`: what the row on screen says, or the dot follows the selection.
    pub dirty: bool,
    /// An unmerged path sits in the index: read from the index alone, no worktree scan.
    pub conflicted: bool,
}

/// Never fails: a read that fails leaves its field at the answer that claims nothing.
#[must_use]
pub fn pulse(root: &Path) -> RepoPulse {
    let started = std::time::Instant::now();
    let Ok(handle) = RepoHandle::open_root(root) else {
        return RepoPulse {
            missing: true,
            ..RepoPulse::default()
        };
    };
    let mut found = RepoPulse::default();
    match handle.head() {
        Ok(crate::Head::Branch { name, oid }) => {
            handle.track(&name, &oid, &mut found);
            found.branch = Some(name);
        }
        Ok(crate::Head::Detached { oid }) => found.branch_at = Some(handle.detached_name(&oid)),
        _ => {}
    }
    found.dirty = handle.has_changes().unwrap_or_else(|err| {
        tracing::error!(error = ?err, context = "pulse: status");
        false
    });
    found.conflicted = handle.has_conflicts().unwrap_or_else(|err| {
        tracing::error!(error = ?err, context = "pulse: conflicts");
        false
    });
    tracing::debug!(
        root = %root.display(),
        elapsed_ms = crate::runner::elapsed_ms(started),
        dirty = found.dirty,
        "pulse read"
    );
    found
}

/// No terminal, no askpass (set and empty, git skips `core.askPass` too), no Credential
/// Manager window, no SSH prompt; a helper answering from its store still answers.
pub(crate) const QUIET: &[(&str, &str)] = &[
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS_REQUIRE", "never"),
    ("GCM_INTERACTIVE", "never"),
];

/// A server that took the connection and fell silent held the check for as long as it liked
/// (R-482): under a byte a second for 60 s ends an HTTP transfer, three keep-alives left
/// unanswered 15 s apart end an SSH one.
pub(crate) const STALL_LIMITS: &[&str] =
    &["-c", "http.lowSpeedLimit=1", "-c", "http.lowSpeedTime=60"];

pub(crate) const BATCH_SSH: &str =
    "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=15";

/// Names the variable that makes Cogit's own executable refuse an ssh askpass request: it
/// is the `SSH_ASKPASS` helper that answers every question with a failure (GR-06).
pub const ASKPASS_REFUSAL_VAR: &str = "COGIT_ASKPASS_REFUSE";

/// For a command whose ssh is one the user chose (`core.sshCommand`), which `BATCH_SSH`
/// cannot reach: ssh asks its questions in a console nobody sees under `CREATE_NO_WINDOW`
/// and the command waits for them, so they are answered with a refusal. A helper the user
/// set up themselves (`SSH_ASKPASS` already in the environment) is left alone.
pub(crate) fn askpass_refusal(own_ssh_command: bool) -> Vec<(String, String)> {
    if !own_ssh_command || std::env::var_os("SSH_ASKPASS").is_some() {
        return Vec::new();
    }
    let Ok(this) = std::env::current_exe() else {
        return Vec::new();
    };
    vec![
        (
            "SSH_ASKPASS".to_owned(),
            this.to_string_lossy().into_owned(),
        ),
        ("SSH_ASKPASS_REQUIRE".to_owned(), "force".to_owned()),
        (ASKPASS_REFUSAL_VAR.to_owned(), "1".to_owned()),
    ]
}

/// `QUIET`, then what overrides it: a later value wins.
pub(crate) fn quiet_env(refusal: &[(String, String)]) -> Vec<(&str, &str)> {
    let mut env = QUIET.to_vec();
    env.extend(
        refusal
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    env
}

impl RepoHandle {
    /// Only the remote-tracking refs move: no prune, no submodules, no maintenance.
    pub fn background_fetch(&self) -> Result<()> {
        let mut args = self.background_options();
        args.extend([
            "fetch",
            "--all",
            "--quiet",
            "--no-auto-gc",
            "--recurse-submodules=no",
        ]);
        let refusal = askpass_refusal(self.has_own_ssh_command());
        self.run_git_with_env(&args, &quiet_env(&refusal)).map(drop)
    }

    /// The server's branch tips, as `git ls-remote --heads` lists them; nothing is written.
    pub fn remote_heads(&self, remote: &str) -> Result<Vec<(String, String)>> {
        self.heads_matching(remote, &[], &[])
    }

    /// Read whole, not through the journal's record: that one is cut past 20 000 lines and
    /// has its secrets masked (R-280).
    fn heads_matching(
        &self,
        remote: &str,
        patterns: &[&str],
        auth: &[(String, String)],
    ) -> Result<Vec<(String, String)>> {
        let mut args = self.background_options();
        args.extend(["ls-remote", "--heads", remote]);
        args.extend(patterns);
        let refusal = askpass_refusal(self.has_own_ssh_command());
        let mut env = quiet_env(&refusal);
        env.extend(
            auth.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let out = self.read_git_with(&args, &env)?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let (oid, name) = line.split_once('\t')?;
                Some((name.trim().to_owned(), oid.trim().to_owned()))
            })
            .collect())
    }

    /// Whether the server's tip of HEAD's upstream is missing from HEAD (R-354); a tracking
    /// ref the server moved, back too, is fetched to its tip (R-921). `None`: no upstream,
    /// or no such branch there.
    /// `credential`: what Cogit has stored for the host, never a question (F-082).
    pub fn pull_probe(
        &self,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
    ) -> Result<Option<bool>> {
        let crate::Head::Branch { name, oid } = self.head()? else {
            return Ok(None);
        };
        let Ok(full) = gix::refs::FullName::try_from(format!("refs/heads/{name}")) else {
            return Ok(None);
        };
        let Some(remote) = self
            .repo
            .branch_remote_name(full.as_ref().shorten(), gix::remote::Direction::Fetch)
        else {
            return Ok(None);
        };
        let Some(Ok(merge)) = self
            .repo
            .branch_remote_ref_name(full.as_ref(), gix::remote::Direction::Fetch)
        else {
            return Ok(None);
        };
        let remote = remote.as_bstr().to_string();
        if !self.background_fetch_of(&remote) {
            return Ok(None);
        }
        let merge = merge.as_bstr().to_string();
        let auth = self.auth_env(&remote, gix::remote::Direction::Fetch, credential);
        let Some(tip) = self
            .heads_matching(&remote, &[&merge], &auth)?
            .into_iter()
            .find_map(|(name, tip)| (name == merge).then_some(tip))
        else {
            return Ok(None);
        };
        self.follow_server(full.as_ref(), &remote, &merge, &tip, &auth);
        let (Ok(tip), Ok(head)) = (
            gix::ObjectId::from_hex(tip.as_bytes()),
            gix::ObjectId::from_hex(oid.as_bytes()),
        ) else {
            return Ok(None);
        };
        if self.repo.find_header(tip).is_err() {
            return Ok(Some(true));
        }
        let base = self.repo.merge_base(head, tip).map(gix::Id::detach);
        Ok(Some(!matches!(base, Ok(base) if base == tip)))
    }

    fn follow_server(
        &self,
        branch: &gix::refs::FullNameRef,
        remote: &str,
        merge: &str,
        tip: &str,
        auth: &[(String, String)],
    ) {
        let Some(Ok(tracking)) = self
            .repo
            .branch_remote_tracking_ref_name(branch, gix::remote::Direction::Fetch)
        else {
            return;
        };
        let tracking = tracking.as_bstr().to_string();
        let known = self
            .repo
            .try_find_reference(tracking.as_str())
            .ok()
            .flatten()
            .and_then(|mut reference| reference.peel_to_id().ok())
            .map(|id| id.to_string());
        if known.as_deref() == Some(tip) {
            return;
        }
        let refspec = format!("+{merge}:{tracking}");
        let mut args = self.background_options();
        args.extend([
            "fetch",
            "--quiet",
            "--no-auto-gc",
            "--no-write-fetch-head",
            "--recurse-submodules=no",
            remote,
            &refspec,
        ]);
        let refusal = askpass_refusal(self.has_own_ssh_command());
        let mut env = quiet_env(&refusal);
        env.extend(
            auth.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        if let Err(err) = self.run_git_with_env(&args, &env) {
            tracing::error!(error = ?err, context = "fetch of an upstream the server moved");
        }
    }

    pub(crate) fn background_options(&self) -> Vec<&'static str> {
        let mut args = STALL_LIMITS.to_vec();
        if !self.has_own_ssh_command() {
            args.extend(["-c", BATCH_SSH]);
        }
        args
    }

    pub(crate) fn has_own_ssh_command(&self) -> bool {
        std::env::var_os("GIT_SSH_COMMAND").is_some()
            || std::env::var_os("GIT_SSH").is_some()
            || self
                .repo
                .config_snapshot()
                .string("core.sshCommand")
                .is_some()
    }

    fn track(&self, name: &str, oid: &str, into: &mut RepoPulse) {
        let Ok(full) = gix::refs::FullName::try_from(format!("refs/heads/{name}")) else {
            return;
        };
        let Some(Ok(tracking)) = self
            .repo
            .branch_remote_tracking_ref_name(full.as_ref(), gix::remote::Direction::Fetch)
        else {
            return;
        };
        let Ok(mut reference) = self.repo.find_reference(tracking.as_ref()) else {
            return;
        };
        let (Ok(upstream), Ok(local)) = (
            reference.peel_to_id(),
            gix::ObjectId::from_hex(oid.as_bytes()),
        ) else {
            return;
        };
        if let Some((ahead, behind)) = self.count_divergence(local, upstream.detach()) {
            into.tracked = true;
            into.ahead = ahead;
            into.behind = behind;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_fetch_asks_nobody() {
        let quiet: std::collections::HashMap<_, _> = QUIET.iter().copied().collect();
        assert_eq!(quiet.get("GIT_TERMINAL_PROMPT"), Some(&"0"));
        assert_eq!(quiet.get("GIT_ASKPASS"), Some(&""));
        assert_eq!(quiet.get("GCM_INTERACTIVE"), Some(&"never"));
        assert!(BATCH_SSH.contains("BatchMode=yes"));
    }
}
