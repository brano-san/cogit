//! The light status of a row of the Repositories list that is not on screen (R-353).

use crate::{RepoHandle, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoPulse {
    pub missing: bool,
    pub branch: Option<String>,
    /// HEAD's branch has an upstream and its remote-tracking ref exists locally.
    pub tracked: bool,
    pub ahead: u32,
    pub behind: u32,
    /// Tracked files, staged changes, conflicts; untracked ones take a directory walk.
    pub dirty: bool,
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
    if let Ok(crate::Head::Branch { name, oid }) = handle.head() {
        handle.track(&name, &oid, &mut found);
        found.branch = Some(name);
    }
    found.dirty = handle.changed_cheaply();
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
const QUIET: &[(&str, &str)] = &[
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS_REQUIRE", "never"),
    ("GCM_INTERACTIVE", "never"),
];

/// A server that took the connection and fell silent held the check for as long as it liked
/// (R-482): under a byte a second for 60 s ends an HTTP transfer, three keep-alives left
/// unanswered 15 s apart end an SSH one.
const STALL_LIMITS: &[&str] = &["-c", "http.lowSpeedLimit=1", "-c", "http.lowSpeedTime=60"];

const BATCH_SSH: &str =
    "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=15";

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
        self.run_git_with_env(&args, QUIET).map(drop)
    }

    /// The server's branch tips, as `git ls-remote --heads` lists them; nothing is written.
    pub fn remote_heads(&self, remote: &str) -> Result<Vec<(String, String)>> {
        self.heads_matching(remote, &[])
    }

    /// Read whole, not through the journal's record: that one is cut past 20 000 lines and
    /// has its secrets masked (R-280).
    fn heads_matching(&self, remote: &str, patterns: &[&str]) -> Result<Vec<(String, String)>> {
        let mut args = self.background_options();
        args.extend(["ls-remote", "--heads", remote]);
        args.extend(patterns);
        let out = self.read_git_with(&args, QUIET)?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let (oid, name) = line.split_once('\t')?;
                Some((name.trim().to_owned(), oid.trim().to_owned()))
            })
            .collect())
    }

    /// Whether the server's tip of HEAD's upstream is missing from HEAD, asked of the
    /// server without a fetch (R-354). `None`: no upstream, or no such branch there.
    pub fn pull_probe(&self) -> Result<Option<bool>> {
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
        let Some(tip) = self
            .heads_matching(&remote, &[&merge])?
            .into_iter()
            .find_map(|(name, tip)| (name == merge).then_some(tip))
        else {
            return Ok(None);
        };
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

    fn background_options(&self) -> Vec<&'static str> {
        let mut args = STALL_LIMITS.to_vec();
        if !self.has_own_ssh_command() {
            args.extend(["-c", BATCH_SSH]);
        }
        args
    }

    fn has_own_ssh_command(&self) -> bool {
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

    fn changed_cheaply(&self) -> bool {
        if self.repo.is_bare() {
            return false;
        }
        let Ok(index) = self.repo.open_index() else {
            return false;
        };
        worktree_moved(self.root(), &index) || self.staged(&index)
    }

    fn staged(&self, index: &gix::index::State) -> bool {
        let Ok(tree) = self.repo.head_tree_id() else {
            return !index.entries().is_empty();
        };
        // Written by commit and checkout; valid, it answers without reading a tree.
        if let Some(cached) = index.tree()
            && cached.num_entries.is_some()
        {
            return cached.id != tree.detach();
        }
        let mut any = false;
        let compared = self.repo.tree_index_status(
            &tree,
            index,
            None,
            gix::status::tree_index::TrackRenames::Disabled,
            |_, _, _| {
                any = true;
                Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Break(()))
            },
        );
        if let Err(err) = compared {
            tracing::error!(error = ?err, context = "pulse: index against HEAD");
        }
        any
    }
}

/// Size and modification time, as the index recorded them. A racily clean entry whose
/// stat matches is taken as clean: telling needs its content hashed.
fn worktree_moved(root: &Path, index: &gix::index::State) -> bool {
    use gix::index::entry::{Flags, Stage, Stat};
    for entry in index.entries() {
        if entry.stage() != Stage::Unconflicted {
            return true;
        }
        if entry.mode.is_submodule()
            || entry.mode.is_sparse()
            || entry
                .flags
                .intersects(Flags::SKIP_WORKTREE | Flags::ASSUME_VALID | Flags::INTENT_TO_ADD)
        {
            continue;
        }
        let path = root.join(gix::path::from_bstr(entry.path(index)));
        let Ok(meta) = gix::index::fs::Metadata::from_path_no_follow(&path) else {
            return true;
        };
        let Ok(now) = Stat::from_fs(&meta) else {
            return true;
        };
        let was = entry.stat;
        // Nanoseconds only where the index kept them: a git that writes none is not a change.
        if was.size != now.size
            || was.mtime.secs != now.mtime.secs
            || (was.mtime.nsecs != 0 && was.mtime.nsecs != now.mtime.nsecs)
        {
            return true;
        }
    }
    false
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
