use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

/// One checkout: the main one cannot be removed, a linked one can be locked or left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub path: String,
    pub name: String,
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
    /// Submodules checked out in it: git removes such a worktree only with `--force`, which
    /// deletes their repositories too.
    pub has_submodules: bool,
}

impl RepoHandle {
    pub fn worktrees(&self) -> Result<Vec<WorktreeEntry>> {
        self.listed(true)
    }

    /// Branches read from each record's `HEAD`, as git reads them before it refuses a branch
    /// held elsewhere: no status walk, no `dirty` and `has_submodules` (R-487).
    pub fn worktree_heads(&self) -> Result<Vec<WorktreeEntry>> {
        self.listed(false)
    }

    fn listed(&self, full: bool) -> Result<Vec<WorktreeEntry>> {
        let here = normalise(self.root());
        let main = self.main_root();
        let mut entries = vec![if full {
            self.describe(main, true, None, &here)
        } else {
            self.outline(main, true, None, &here, self.repo.common_dir())
        }];

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
            let mut entry = match proxy.base() {
                Ok(path) if full => self.describe(path, false, locked, &here),
                Ok(path) => self.outline(path, false, locked, &here, proxy.git_dir()),
                Err(_) => WorktreeEntry {
                    path: proxy.git_dir().display().to_string().replace('\\', "/"),
                    name: proxy.id().to_string(),
                    branch: None,
                    head: String::new(),
                    is_main: false,
                    is_current: false,
                    locked,
                    missing: true,
                    dirty: false,
                    has_submodules: false,
                },
            };
            // The folder is gone but its record is not, and git still keeps the branch there.
            if entry.missing {
                (entry.branch, entry.head) = self.recorded_head(proxy.git_dir());
            }
            entries.push(entry);
        }
        // gix orders the records by name, git by path — case-folded under core.ignoreCase.
        let fold = self
            .repo
            .config_snapshot()
            .boolean("core.ignoreCase")
            .unwrap_or(false);
        entries[1..].sort_by(|a, b| {
            if fold {
                a.path
                    .to_ascii_lowercase()
                    .cmp(&b.path.to_ascii_lowercase())
            } else {
                a.path.cmp(&b.path)
            }
        });
        Ok(entries)
    }

    fn recorded_head(&self, record: &std::path::Path) -> (Option<String>, String) {
        let Ok(text) = std::fs::read_to_string(record.join("HEAD")) else {
            return (None, String::new());
        };
        let text = text.trim();
        let Some(full) = text.strip_prefix("ref: ") else {
            return (None, text.to_owned());
        };
        let head = self
            .repo
            .find_reference(full)
            .ok()
            .and_then(|mut reference| reference.peel_to_id().ok())
            .map(|id| id.to_string())
            .unwrap_or_default();
        let branch = full.strip_prefix("refs/heads/").unwrap_or(full).to_owned();
        (Some(branch), head)
    }

    /// The worktree holding this branch, unless it is the current one (T3.8).
    pub fn worktree_holding(&self, branch: &str) -> Result<Option<WorktreeEntry>> {
        let Some(held) = self
            .worktree_heads()?
            .into_iter()
            .find(|entry| !entry.is_current && entry.branch.as_deref() == Some(branch))
        else {
            return Ok(None);
        };
        if held.missing {
            return Ok(Some(held));
        }
        let here = normalise(self.root());
        let path = std::path::PathBuf::from(&held.path);
        let mut entry = self.describe(path, held.is_main, held.locked.clone(), &here);
        if entry.missing {
            (entry.branch, entry.head) = (held.branch, held.head);
        }
        Ok(Some(entry))
    }

    pub fn add_worktree(&self, path: &str, branch: &str, create: bool) -> Result<()> {
        self.add_worktree_at(path, branch, create, None)
    }

    /// `worktree add` takes the path last with `-b` and first otherwise; `base` defaults to HEAD.
    pub fn add_worktree_at(
        &self,
        path: &str,
        branch: &str,
        create: bool,
        base: Option<&str>,
    ) -> Result<()> {
        let mut args = if create {
            vec!["worktree", "add", "-b", branch, path]
        } else {
            vec!["worktree", "add", path, branch]
        };
        if let (true, Some(base)) = (create, base) {
            args.push(base);
        }
        self.run_git(&args).map(drop)
    }

    pub fn lock_worktree(&self, path: &str, reason: Option<&str>) -> Result<()> {
        let mut args = vec!["worktree", "lock"];
        if let Some(reason) = reason.filter(|reason| !reason.trim().is_empty()) {
            args.extend(["--reason", reason]);
        }
        args.push(path);
        self.run_git(&args).map(drop)
    }

    pub fn unlock_worktree(&self, path: &str) -> Result<()> {
        self.run_git(&["worktree", "unlock", path]).map(drop)
    }

    /// `prune` cannot name one entry; `remove --force` on a missing folder deletes only the
    /// record, and the check keeps it from ever deleting a folder that is there (R-184).
    pub fn prune_worktree(&self, path: &str) -> Result<()> {
        if std::path::Path::new(path).exists() {
            return Err(GitError::InvalidState(format!(
                "{path} still exists; remove the worktree instead of pruning it"
            )));
        }
        self.run_git(&["worktree", "remove", "--force", path])
            .map(drop)
    }

    /// A folder moved to another disk keeps its index but not its files' stat data, so
    /// every status reads each file again until git refreshes the index; git status would,
    /// gix never writes it. Refreshed here, once, while the repair holds the queue.
    pub fn repair_worktree(&self, path: &str) -> Result<()> {
        self.run_git(&["worktree", "repair", path])?;
        let started = std::time::Instant::now();
        for folder in [std::path::Path::new(path), self.root()] {
            let refreshed = RepoHandle::open_exact(folder).and_then(|handle| {
                if handle.repo.is_bare() {
                    return Ok(());
                }
                handle
                    .run_git(&[
                        "update-index",
                        "-q",
                        "--unmerged",
                        "--ignore-submodules",
                        "--refresh",
                    ])
                    .map(drop)
            });
            if let Err(err) = refreshed {
                tracing::error!(error = ?err, context = "refresh the index after a worktree repair");
            }
        }
        tracing::info!(elapsed = ?started.elapsed(), "worktree indexes refreshed after repair");
        Ok(())
    }

    pub fn worktree_changes(&self, path: &str) -> Result<Vec<crate::FileEntry>> {
        let files = self.linked_handle(path)?.worktree_files()?;
        let mut seen = std::collections::BTreeSet::new();
        Ok(files
            .staged
            .into_iter()
            .chain(files.unstaged)
            .filter(|file| seen.insert(file.path.clone()))
            .collect())
    }

    /// Uncommitted work, untracked included, into a stash that outlives the worktree.
    pub fn stash_worktree_changes(&self, path: &str, message: &str) -> Result<Option<String>> {
        self.linked_handle(path)?
            .stash_paths(&[".".to_owned()], message)
    }

    /// Around the shared `.git`, which a linked worktree reports as `.git/worktrees/<n>/../..`.
    fn main_root(&self) -> std::path::PathBuf {
        let mut common = std::path::PathBuf::new();
        for part in self.repo.common_dir().components() {
            match part {
                std::path::Component::ParentDir => {
                    common.pop();
                }
                std::path::Component::CurDir => {}
                other => common.push(other),
            }
        }
        match common.parent() {
            Some(parent) if common.file_name().is_some_and(|name| name == ".git") => {
                parent.to_path_buf()
            }
            // A bare repository is its own folder, whatever it is called; asked from one of
            // its worktrees, falling back to the asking root listed that one twice.
            _ if gix::open(&common).is_ok_and(|repo| repo.is_bare()) => common,
            _ => self.root().to_path_buf(),
        }
    }

    fn linked_handle(&self, path: &str) -> Result<RepoHandle> {
        let wanted = normalise(std::path::Path::new(path));
        if !self
            .worktree_heads()?
            .iter()
            .any(|entry| entry.path == wanted && !entry.missing)
        {
            return Err(GitError::InvalidState(format!(
                "{path} is not a worktree of this repository"
            )));
        }
        RepoHandle::open_exact(std::path::Path::new(path))
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

    /// See [`Self::worktree_heads`]; `record` is the git directory whose `HEAD` it reads.
    fn outline(
        &self,
        path: std::path::PathBuf,
        is_main: bool,
        locked: Option<String>,
        here: &str,
        record: &std::path::Path,
    ) -> WorktreeEntry {
        let mut entry = unread(&path, is_main, locked, here);
        if !entry.missing {
            (entry.branch, entry.head) = self.recorded_head(record);
        }
        entry
    }

    fn describe(
        &self,
        path: std::path::PathBuf,
        is_main: bool,
        locked: Option<String>,
        here: &str,
    ) -> WorktreeEntry {
        let mut entry = unread(&path, is_main, locked, here);
        // Discovery would climb to whatever repository holds the parent folder.
        if entry.missing {
            return entry;
        }
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
        if let Ok(dirty) = handle.has_changes() {
            entry.dirty = dirty;
        }
        entry.has_submodules = !is_main && handle.checks_out_submodules();
        entry
    }

    /// Git's own test before it refuses to remove a worktree: a `modules` folder in its git
    /// directory, or a gitlink of its index with a repository in the folder.
    fn checks_out_submodules(&self) -> bool {
        if self.repo.git_dir().join("modules").is_dir() {
            return true;
        }
        let Ok(index) = self.current_index() else {
            return false;
        };
        index.entries().iter().any(|entry| {
            entry.mode.is_submodule()
                && self
                    .root()
                    .join(gix::path::from_bstr(entry.path(&index)))
                    .join(".git")
                    .exists()
        })
    }
}

fn unread(
    path: &std::path::Path,
    is_main: bool,
    locked: Option<String>,
    here: &str,
) -> WorktreeEntry {
    let shown = normalise(path);
    WorktreeEntry {
        is_current: shown == here,
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| shown.clone()),
        path: shown,
        branch: None,
        head: String::new(),
        is_main,
        locked,
        // Git's own test for a linked one is its `.git` file: a folder left without it
        // is prunable, and opening it would find the main repository around it.
        missing: if is_main {
            !path.is_dir()
        } else {
            !path.join(".git").exists()
        },
        dirty: false,
        has_submodules: false,
    }
}

fn normalise(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}
