use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const DISABLED: &str = ".disabled";

/// Directories a team keeps versioned hooks in, in the order Cogit suggests them.
const CANDIDATE_DIRS: &[&str] = &[".githooks", ".hooks", "hooks"];

/// Git's own list, githooks(5) in its order, with the moment each one runs. The
/// description is the whole point of the inspector: hooks are avoided because nobody
/// remembers when they fire.
const HOOKS: &[(&str, &str)] = &[
    (
        "applypatch-msg",
        "git am, with the patch's message; can edit it or refuse the patch",
    ),
    (
        "pre-applypatch",
        "git am, after the patch is applied, before it is committed",
    ),
    (
        "post-applypatch",
        "git am, after the commit is made; cannot cancel anything",
    ),
    (
        "pre-commit",
        "Before a commit is created; a non-zero exit cancels it",
    ),
    (
        "pre-merge-commit",
        "Before a merge commit is created; a non-zero exit cancels the merge",
    ),
    (
        "prepare-commit-msg",
        "After the message template is built, before the editor opens",
    ),
    (
        "commit-msg",
        "With the finished message; the usual place to enforce a format",
    ),
    (
        "post-commit",
        "After a commit is written; cannot cancel anything",
    ),
    ("pre-rebase", "Before a rebase starts"),
    (
        "post-checkout",
        "After checkout or a clone's initial checkout",
    ),
    ("post-merge", "After a merge that changed the working tree"),
    (
        "pre-push",
        "After the remote is contacted, before anything is sent",
    ),
    ("pre-receive", "On the server, once per push"),
    ("update", "On the server, once per ref being updated"),
    (
        "proc-receive",
        "On the server, for the refs a push hands it to update itself",
    ),
    ("post-receive", "On the server, after a push is accepted"),
    (
        "post-update",
        "On the server, after a push has updated its refs",
    ),
    (
        "reference-transaction",
        "Whenever refs are updated: prepared, committed or aborted",
    ),
    (
        "push-to-checkout",
        "On the server, when a push updates its checked-out branch",
    ),
    (
        "pre-auto-gc",
        "Before Git runs housekeeping in the background",
    ),
    (
        "post-rewrite",
        "After a commit is replaced by amend or rebase",
    ),
    (
        "sendemail-validate",
        "git send-email, once per patch before it is sent",
    ),
    (
        "fsmonitor-watchman",
        "When core.fsmonitor names it: asked which files changed",
    ),
    (
        "p4-changelist",
        "git p4 submit, with the changelist text to check",
    ),
    (
        "p4-prepare-changelist",
        "git p4 submit, after the changelist message is built",
    ),
    ("p4-post-changelist", "git p4 submit, after it succeeded"),
    ("p4-pre-submit", "git p4 submit, before it starts"),
    ("post-index-change", "After the index is written"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HookState {
    Missing,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HookSource {
    GitHooks,
    HooksPath,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
    pub name: String,
    pub description: String,
    pub state: HookState,
    pub source: Option<HookSource>,
    pub executable: bool,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HookOverview {
    /// Where Git will actually look right now.
    pub active_dir: String,
    pub source: HookSource,
    /// `core.hooksPath` exactly as configured, so the UI can show what to fix.
    pub configured_path: Option<String>,
    /// A versioned hook directory sitting in the tree that nothing points at yet.
    pub available_path: Option<String>,
    pub hooks: Vec<Hook>,
}

/// Whether a name is one of Git's hooks; a preset target is checked against it.
#[must_use]
pub fn is_hook_name(name: &str) -> bool {
    is_known(name)
}

fn is_known(name: &str) -> bool {
    HOOKS.iter().any(|(hook, _)| *hook == name)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

/// NTFS has no execution bit, and Git for Windows runs hooks through bash regardless.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.exists()
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

impl RepoHandle {
    /// A pathname setting as git reads it, `~/` expanded to the home folder. A value that
    /// cannot be expanded is taken as written.
    pub(crate) fn pathname_setting(&self, key: &str) -> Option<PathBuf> {
        let config = self.repo.config_snapshot();
        match config.trusted_path(key) {
            Ok(Some(path)) => Some(path),
            Ok(None) => None,
            Err(err) => {
                tracing::warn!(key, error = %err, "a path in the config could not be expanded");
                config.string(key).map(|raw| PathBuf::from(raw.to_string()))
            }
        }
    }

    /// `core.hooksPath` as written, relative or absolute.
    #[must_use]
    pub fn configured_hooks_path(&self) -> Option<String> {
        let value = self.repo.config_snapshot().string("core.hooksPath")?;
        let text = value.to_string();
        (!text.trim().is_empty()).then_some(text)
    }

    pub(crate) fn hooks_dir(&self) -> (PathBuf, HookSource) {
        match self.configured_hooks_path() {
            Some(_) => {
                let path = self.pathname_setting("core.hooksPath").unwrap_or_default();
                let resolved = if path.is_absolute() {
                    path
                } else {
                    self.root().join(path)
                };
                (resolved, HookSource::HooksPath)
            }
            None => (self.common_dir().join("hooks"), HookSource::GitHooks),
        }
    }

    pub(crate) fn hook_path(&self, name: &str, enabled: bool) -> Result<PathBuf> {
        if !is_known(name) {
            return Err(GitError::InvalidState(format!("{name} is not a Git hook")));
        }
        let (dir, _) = self.hooks_dir();
        Ok(dir.join(if enabled {
            name.to_owned()
        } else {
            format!("{name}{DISABLED}")
        }))
    }

    pub fn hooks(&self) -> Result<HookOverview> {
        let (dir, source) = self.hooks_dir();

        let hooks = HOOKS
            .iter()
            .map(|(name, description)| {
                let enabled = dir.join(name);
                let disabled = dir.join(format!("{name}{DISABLED}"));
                let (state, path) = if enabled.is_file() {
                    (HookState::Enabled, Some(enabled))
                } else if disabled.is_file() {
                    (HookState::Disabled, Some(disabled))
                } else {
                    (HookState::Missing, None)
                };

                Hook {
                    name: (*name).to_owned(),
                    description: (*description).to_owned(),
                    state,
                    source: (state != HookState::Missing).then_some(source),
                    executable: path.as_deref().is_some_and(is_executable),
                    size: path
                        .as_deref()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map_or(0, |meta| u32::try_from(meta.len()).unwrap_or(u32::MAX)),
                }
            })
            .collect();

        Ok(HookOverview {
            active_dir: dir.to_string_lossy().replace('\\', "/"),
            source,
            configured_path: self.configured_hooks_path(),
            available_path: self.available_hooks_path(),
            hooks,
        })
    }

    /// A directory of hooks the repository ships that `core.hooksPath` does not point at:
    /// the usual reason a teammate's hooks silently do nothing (doc/modules/M10-hooks.md).
    fn available_hooks_path(&self) -> Option<String> {
        if self.configured_hooks_path().is_some() {
            return None;
        }
        CANDIDATE_DIRS
            .iter()
            .find(|name| {
                let dir = self.root().join(name);
                dir.is_dir()
                    && HOOKS.iter().any(|(hook, _)| {
                        dir.join(hook).is_file() || dir.join(format!("{hook}{DISABLED}")).is_file()
                    })
            })
            .map(|name| (*name).to_owned())
    }

    pub fn read_hook(&self, name: &str) -> Result<String> {
        let enabled = self.hook_path(name, true)?;
        let path = if enabled.is_file() {
            enabled
        } else {
            self.hook_path(name, false)?
        };
        let bytes = std::fs::read(&path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Always LF: a CRLF shebang makes the kernel look for an interpreter named `sh\r`.
    pub fn write_hook(&self, name: &str, body: &str) -> Result<()> {
        let path = self.hook_path(name, true)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, body.replace("\r\n", "\n"))?;
        make_executable(&path)?;
        let _ = std::fs::remove_file(self.hook_path(name, false)?);
        Ok(())
    }

    /// Renaming, not deleting: turning a hook off must never lose the script.
    pub fn set_hook_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let (from, to) = if enabled {
            (self.hook_path(name, false)?, self.hook_path(name, true)?)
        } else {
            (self.hook_path(name, true)?, self.hook_path(name, false)?)
        };
        if !from.is_file() {
            return Err(GitError::InvalidState(format!(
                "{name} is not present in the active hooks directory"
            )));
        }
        std::fs::rename(&from, &to)?;
        if enabled {
            make_executable(&to)?;
        }
        Ok(())
    }
}

/// Plausible arguments so a hook that reads them does something useful in a dry run.
impl RepoHandle {
    /// A hooks directory checked out with CRLF has a shebang the kernel cannot read, so
    /// the team setup offers the `.gitattributes` rule (doc/modules/M10-hooks.md, T10.6).
    pub fn needs_eol_rule(&self, dir: &str) -> Result<bool> {
        let rule = format!("{dir}/**");
        // Bytes, not UTF-8: a comment in another code page must not hide the rules.
        let Some(bytes) = self.gitattributes()? else {
            return Ok(true);
        };
        Ok(!String::from_utf8_lossy(&bytes)
            .lines()
            .any(|line| line.starts_with(&rule) && line.contains("eol=lf")))
    }

    /// Appended, never rewritten: the file is the team's, in whatever encoding they saved.
    pub fn add_eol_rule(&self, dir: &str) -> Result<()> {
        if !self.needs_eol_rule(dir)? {
            return Ok(());
        }

        let existing = self.gitattributes()?.unwrap_or_default();
        let mut addition = String::new();
        if !existing.is_empty() && !existing.ends_with(b"\n") {
            addition.push('\n');
        }
        addition.push_str(&format!("{dir}/** eol=lf\n"));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root().join(".gitattributes"))?;
        std::io::Write::write_all(&mut file, addition.as_bytes())?;
        Ok(())
    }

    fn gitattributes(&self) -> Result<Option<Vec<u8>>> {
        match std::fs::read(self.root().join(".gitattributes")) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}
