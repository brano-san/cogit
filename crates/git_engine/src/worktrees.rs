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
    /// Changed paths, staged or not, each counted once; with `untracked`, what `dirty` sums.
    pub changed: u32,
    pub untracked: u32,
    /// The main entry of a bare repository: a git directory with no files checked out.
    pub bare: bool,
    /// Submodules checked out in it: git removes such a worktree only with `--force`, which
    /// deletes their repositories too.
    pub has_submodules: bool,
}

/// What a new worktree checks out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorktreeBranch {
    /// A new branch at `start` (HEAD when `None`); `track` follows `start`, a remote branch.
    New {
        name: String,
        start: Option<String>,
        track: bool,
    },
    Existing {
        name: String,
    },
    Detached {
        start: Option<String>,
    },
}

impl WorktreeBranch {
    /// `worktree add` takes the path last with `-b` and first otherwise.
    fn add_args(&self, path: &str) -> Vec<String> {
        let mut args: Vec<String> = ["worktree", "add"].map(str::to_owned).into();
        match self {
            Self::New { name, start, track } => {
                // Git tracks a remote start point by default; the box decides instead.
                args.push(if *track { "--track" } else { "--no-track" }.into());
                args.extend(["-b".into(), name.clone(), path.into()]);
                args.extend(start.clone());
            }
            Self::Existing { name } => args.extend([path.into(), name.clone()]),
            Self::Detached { start } => {
                args.extend(["--detach".into(), path.into()]);
                args.extend(start.clone());
            }
        }
        args
    }
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
                    path: crate::slash_path(proxy.git_dir()),
                    name: proxy.id().to_string(),
                    branch: None,
                    head: String::new(),
                    is_main: false,
                    is_current: false,
                    locked,
                    missing: true,
                    dirty: false,
                    changed: 0,
                    untracked: 0,
                    bare: false,
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

    /// `base` defaults to HEAD.
    pub fn add_worktree_at(
        &self,
        path: &str,
        branch: &str,
        create: bool,
        base: Option<&str>,
    ) -> Result<()> {
        let branch = if create {
            WorktreeBranch::New {
                name: branch.to_owned(),
                start: base.map(str::to_owned),
                track: false,
            }
        } else {
            WorktreeBranch::Existing {
                name: branch.to_owned(),
            }
        };
        self.add_worktree_with(path, &branch)
    }

    pub fn add_worktree_with(&self, path: &str, branch: &WorktreeBranch) -> Result<()> {
        let args = branch.add_args(path);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
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

    /// Uncommitted work, untracked included, into a backup that outlives the worktree: its
    /// ref lives in the common git directory.
    pub fn stash_worktree_changes(&self, path: &str, message: &str) -> Result<Option<String>> {
        self.linked_handle(path)?
            .backup_paths(&[".".to_owned()], message)
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

    pub(crate) fn linked_handle(&self, path: &str) -> Result<RepoHandle> {
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

    /// Why this folder must not be deleted as a worktree's leftover, if it must not.
    fn leftover_blocker(&self, path: &str) -> Result<Option<String>> {
        let folder = std::path::Path::new(path);
        if !folder.is_dir() {
            return Ok(Some(format!("{path} is not a folder")));
        }
        let wanted = normalise(folder);
        if self
            .worktree_heads()?
            .iter()
            .any(|entry| entry.path == wanted)
        {
            return Ok(Some(format!(
                "{path} is still a worktree of this repository"
            )));
        }
        if folder.join(".git").is_dir() {
            return Ok(Some(format!("{path} holds a Git repository")));
        }
        let real =
            |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let target = real(folder);
        let guarded = [
            self.root().to_path_buf(),
            self.main_root(),
            self.repo.common_dir().to_path_buf(),
        ];
        if guarded.iter().any(|guard| real(guard).starts_with(&target)) {
            return Ok(Some(format!("{path} is, or contains, this repository")));
        }
        Ok(None)
    }

    /// A folder git could not finish deleting after it dropped the worktree's record.
    pub fn worktree_leftover(&self, path: &str) -> bool {
        matches!(self.leftover_blocker(path), Ok(None))
    }

    pub fn delete_worktree_leftover(&self, path: &str) -> Result<()> {
        if let Some(reason) = self.leftover_blocker(path)? {
            return Err(GitError::InvalidState(reason));
        }
        std::fs::remove_dir_all(path)
            .map_err(|err| GitError::Io(format!("cannot delete {path}: {err}")))
    }

    /// `force` on a locked worktree passes `--force` twice, git's way past the lock.
    pub fn remove_worktree(&self, path: &str, force: bool) -> Result<()> {
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
            let wanted = normalise(std::path::Path::new(path));
            if self
                .worktree_heads()?
                .iter()
                .any(|entry| entry.path == wanted && entry.locked.is_some())
            {
                args.push("--force");
            }
        }
        args.push(path);
        self.run_git(&args).map(drop)
    }

    /// Git refuses the main worktree, a locked one and one with submodules checked out.
    pub fn move_worktree(&self, path: &str, to: &str) -> Result<()> {
        self.run_git(&["worktree", "move", path, to]).map(drop)
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
        // A linked worktree of a bare repository reads `core.bare = true` too.
        entry.bare = is_main && handle.repo.is_bare();
        match handle.change_counts() {
            Ok((changed, untracked)) => {
                (entry.changed, entry.untracked) = (changed, untracked);
                entry.dirty = changed > 0 || untracked > 0;
            }
            Err(err) => tracing::error!(error = ?err, context = "count a worktree's changes"),
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
        changed: 0,
        untracked: 0,
        bare: false,
        has_submodules: false,
    }
}

/// The folder as git spells it: git lists worktrees by real path, so a symlink or a
/// Windows 8.3 name (`RUNNER~1`) would never equal its own entry.
fn normalise(path: &std::path::Path) -> String {
    crate::slash_path(&portable::real_path(path))
}

/// Git takes a folder that is missing or empty; the dialog asks before it runs `worktree add`.
#[must_use]
pub fn worktree_folder_problem(path: &std::path::Path) -> Option<String> {
    let shown = normalise(path);
    if path.is_file() {
        return Some(format!("{shown} is a file"));
    }
    let occupied = std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_some());
    occupied.then(|| format!("{shown} is not empty"))
}
