use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

/// One checkout of the repository. The main one cannot be removed; a linked one can be
/// locked, or left behind when its folder is deleted (M3 T3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub path: String,
    /// `None` when the worktree is on a detached HEAD.
    pub branch: Option<String>,
    pub head: String,
    pub is_main: bool,
    /// The worktree this handle was opened on.
    pub is_current: bool,
    /// The reason git was given, or an empty string when it was locked without one.
    pub locked: Option<String>,
    pub missing: bool,
    pub dirty: bool,
}

impl RepoHandle {
    /// Read through `gix`: the main worktree plus every linked one git knows about.
    pub fn worktrees(&self) -> Result<Vec<WorktreeEntry>> {
        let here = normalise(self.root());
        let mut entries = vec![self.describe(self.root().to_path_buf(), true, None, &here)];

        let linked = self
            .repo
            .worktrees()
            .map_err(|err| GitError::Io(format!("cannot list worktrees: {err}")))?;
        for proxy in linked {
            let locked = proxy.is_locked().then(|| {
                proxy
                    .lock_reason()
                    .map(|r| r.to_string())
                    .unwrap_or_default()
            });
            // `base()` fails exactly when the checkout is gone, which is what we want to say.
            match proxy.base() {
                Ok(path) => entries.push(self.describe(path, false, locked, &here)),
                Err(_) => entries.push(WorktreeEntry {
                    path: proxy.git_dir().display().to_string().replace('\\', "/"),
                    branch: None,
                    head: String::new(),
                    is_main: false,
                    is_current: false,
                    locked,
                    missing: true,
                    dirty: false,
                }),
            }
        }
        Ok(entries)
    }

    /// The worktree holding this branch, unless it is the one we are in: switching to where
    /// you already are is not a switch (T3.8).
    pub fn worktree_holding(&self, branch: &str) -> Result<Option<WorktreeEntry>> {
        Ok(self
            .worktrees()?
            .into_iter()
            .find(|entry| !entry.is_current && entry.branch.as_deref() == Some(branch)))
    }

    /// `git worktree add` takes the path last when creating a branch and first otherwise.
    pub fn add_worktree(&self, path: &str, branch: &str, create: bool) -> Result<()> {
        let args = if create {
            vec!["worktree", "add", "-b", branch, path]
        } else {
            vec!["worktree", "add", path, branch]
        };
        self.run_git(&args).map(drop)
    }

    pub fn remove_worktree(&self, path: &str, force: bool) -> Result<()> {
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path);
        self.run_git(&args).map(drop)
    }

    pub fn prune_worktrees(&self) -> Result<()> {
        self.run_git(&["worktree", "prune"]).map(drop)
    }

    fn describe(
        &self,
        path: std::path::PathBuf,
        is_main: bool,
        locked: Option<String>,
        here: &str,
    ) -> WorktreeEntry {
        let shown = normalise(&path);
        let mut entry = WorktreeEntry {
            is_current: shown == here,
            path: shown,
            branch: None,
            head: String::new(),
            is_main,
            locked,
            missing: !path.is_dir(),
            dirty: false,
        };

        let Ok(handle) = RepoHandle::open(&path) else {
            entry.missing = true;
            return entry;
        };
        if let Ok(head) = handle.head() {
            match head {
                crate::Head::Branch { name, oid } => {
                    entry.branch = Some(name);
                    entry.head = oid;
                }
                crate::Head::Detached { oid } => entry.head = oid,
                crate::Head::Unborn { .. } => {}
            }
        }
        if let Ok(status) = handle.status() {
            entry.dirty = !status.is_clean();
        }
        entry
    }
}

fn normalise(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}
