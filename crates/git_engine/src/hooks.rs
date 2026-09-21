use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const DISABLED: &str = ".disabled";

/// Directories a team keeps versioned hooks in, in the order Cogit suggests them.
const CANDIDATE_DIRS: &[&str] = &[".githooks", ".hooks", "hooks"];

/// Git's own list, with the moment each one runs. The description is the whole point of
/// the inspector: hooks are avoided because nobody remembers when they fire.
const HOOKS: &[(&str, &str)] = &[
    (
        "pre-commit",
        "Before a commit is created; a non-zero exit cancels it",
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
    ("post-receive", "On the server, after a push is accepted"),
    (
        "post-rewrite",
        "After a commit is replaced by amend or rebase",
    ),
    (
        "pre-auto-gc",
        "Before Git runs housekeeping in the background",
    ),
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
    /// `core.hooksPath` as written, relative or absolute.
    #[must_use]
    pub fn configured_hooks_path(&self) -> Option<String> {
        let value = self.repo.config_snapshot().string("core.hooksPath")?;
        let text = value.to_string();
        (!text.trim().is_empty()).then_some(text)
    }

    fn hooks_dir(&self) -> (PathBuf, HookSource) {
        match self.configured_hooks_path() {
            Some(configured) => {
                let path = PathBuf::from(&configured);
                let resolved = if path.is_absolute() {
                    path
                } else {
                    self.root().join(path)
                };
                (resolved, HookSource::HooksPath)
            }
            None => (self.git_dir().join("hooks"), HookSource::GitHooks),
        }
    }

    fn hook_path(&self, name: &str, enabled: bool) -> Result<PathBuf> {
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
const SAMPLE_MESSAGE: &str = "feat: a sample message from a Cogit dry run\n";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HookRun {
    pub name: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u32,
    /// Over the budget in doc/modules/M10-hooks.md, where a hook starts costing the commit.
    pub slow: bool,
}

const SLOW_MS: u32 = 5_000;

impl RepoHandle {
    /// Runs the hook and nothing else: no commit, no checkout, no index change.
    pub fn run_hook(&self, name: &str) -> Result<HookRun> {
        let path = self.hook_path(name, true)?;
        if !path.is_file() {
            return Err(GitError::InvalidState(format!(
                "{name} is not enabled in the active hooks directory"
            )));
        }

        let scratch = self.git_dir().join("COGIT_HOOK_MSG");
        let args = sample_args(name, &scratch)?;

        let started = std::time::Instant::now();
        let output = hook_command(&path, self.root(), &args).output();
        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let _ = std::fs::remove_file(&scratch);
        let output = output?;

        let run = HookRun {
            name: name.to_owned(),
            exit_code: output.status.code(),
            stdout: crate::GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ),
            stderr: crate::GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ),
            duration_ms,
            slow: duration_ms > SLOW_MS,
        };
        tracing::info!(hook = name, exit = ?run.exit_code, duration_ms, "hook dry run");
        Ok(run)
    }
}

fn sample_args(name: &str, scratch: &Path) -> Result<Vec<String>> {
    let message_file = || -> Result<Vec<String>> {
        std::fs::write(scratch, SAMPLE_MESSAGE)?;
        Ok(vec![scratch.to_string_lossy().into_owned()])
    };

    Ok(match name {
        "commit-msg" => message_file()?,
        "prepare-commit-msg" => {
            let mut args = message_file()?;
            args.push("message".to_owned());
            args
        }
        "pre-push" => vec!["origin".to_owned(), "origin".to_owned()],
        "post-checkout" => vec!["HEAD".to_owned(), "HEAD".to_owned(), "1".to_owned()],
        "post-merge" => vec!["0".to_owned()],
        _ => Vec::new(),
    })
}

/// Git for Windows ships bash, and a hook's `#!` line only means something to a shell.
#[cfg(windows)]
fn hook_command(path: &Path, root: &Path, args: &[String]) -> std::process::Command {
    use std::os::windows::process::CommandExt as _;
    let mut command = std::process::Command::new("bash");
    command.creation_flags(0x0800_0000);
    command.arg(path);
    command.args(args);
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command
}

#[cfg(not(windows))]
fn hook_command(path: &Path, root: &Path, args: &[String]) -> std::process::Command {
    let mut command = std::process::Command::new(path);
    command.args(args);
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command
}

impl RepoHandle {
    /// Runs the user's check command in the repository and reports what it said. A failing
    /// check is a verdict, not an error: only being unable to run one is a failure (T11.3).
    pub fn run_check(&self, command: &str) -> Result<HookRun> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Err(GitError::InvalidState("no check command given".to_owned()));
        }

        let started = std::time::Instant::now();
        let output = shell_command(trimmed, self.root())
            .output()
            .map_err(|err| GitError::Io(format!("cannot run the check: {err}")))?;
        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);

        let run = HookRun {
            name: "check".to_owned(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_ms,
            slow: duration_ms > SLOW_MS,
        };

        if let Some(sink) = self.journal() {
            sink(crate::GitOutput::record(
                format!("check: {trimmed}"),
                run.exit_code,
                &run.stdout,
                &run.stderr,
                duration_ms,
            ));
        }
        Ok(run)
    }
}

/// The same shell the hooks use, so a check reads like the command line the user typed.
#[cfg(windows)]
fn shell_command(command: &str, root: &Path) -> std::process::Command {
    use std::os::windows::process::CommandExt as _;
    let mut spawned = std::process::Command::new("bash");
    spawned.creation_flags(0x0800_0000);
    spawned.args(["-lc", command]);
    spawned.current_dir(root);
    spawned.env("GIT_TERMINAL_PROMPT", "0");
    spawned
}

#[cfg(not(windows))]
fn shell_command(command: &str, root: &Path) -> std::process::Command {
    let mut spawned = std::process::Command::new("sh");
    spawned.args(["-c", command]);
    spawned.current_dir(root);
    spawned.env("GIT_TERMINAL_PROMPT", "0");
    spawned
}

const BYPASS_FILE: &str = "cogit-hook-bypasses";
const SEPARATOR: char = '\u{1f}';

/// Git records nothing about `--no-verify`, so Cogit keeps its own note per clone.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Bypass {
    pub oid: String,
    pub summary: String,
    #[specta(type = specta_typescript::Number)]
    pub at: i64,
}

impl RepoHandle {
    /// Newest first; an unparsable line is skipped, never fatal.
    pub fn bypass_log(&self) -> Result<Vec<Bypass>> {
        let Ok(text) = std::fs::read_to_string(self.git_dir().join(BYPASS_FILE)) else {
            return Ok(Vec::new());
        };

        let mut entries: Vec<Bypass> = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, SEPARATOR);
                let at = parts.next()?.parse().ok()?;
                Some(Bypass {
                    at,
                    oid: parts.next()?.to_owned(),
                    summary: parts.next().unwrap_or_default().to_owned(),
                })
            })
            .collect();
        entries.reverse();
        Ok(entries)
    }

    pub(crate) fn record_bypass(&self, oid: &str, summary: &str) {
        use std::io::Write as _;

        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        let subject = summary.lines().next().unwrap_or_default();
        let line = format!("{at}{SEPARATOR}{oid}{SEPARATOR}{subject}\n");

        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.git_dir().join(BYPASS_FILE))
            .and_then(|mut file| file.write_all(line.as_bytes()));
        if let Err(err) = written {
            tracing::error!(error = ?err, context = "failed to record a hook bypass");
        }

        // Exit zero with a line on stderr is what the Output panel marks as a warning,
        // so a bypass reads there like any other thing worth noticing (M10 T10.5).
        self.journal_entry(crate::GitOutput::record(
            "hooks bypassed".to_owned(),
            Some(0),
            "",
            &format!(
                "warning: {} committed without running the hooks: {subject}",
                &oid[..7.min(oid.len())]
            ),
            0,
        ));
    }
}

impl RepoHandle {
    /// `commit.template` as text, or `None` when it is unset or points nowhere.
    pub fn commit_template(&self) -> Result<Option<String>> {
        let Some(configured) = self.repo.config_snapshot().string("commit.template") else {
            return Ok(None);
        };

        let raw = configured.to_string();
        let path = std::path::Path::new(&raw);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root().join(path)
        };
        Ok(std::fs::read_to_string(resolved).ok())
    }
}

impl RepoHandle {
    /// A hooks directory checked out with CRLF has a shebang the kernel cannot read, so
    /// the team setup offers the `.gitattributes` rule (doc/modules/M10-hooks.md, T10.6).
    pub fn needs_eol_rule(&self, dir: &str) -> Result<bool> {
        let rule = format!("{dir}/**");
        let Ok(text) = std::fs::read_to_string(self.root().join(".gitattributes")) else {
            return Ok(true);
        };
        Ok(!text
            .lines()
            .any(|line| line.starts_with(&rule) && line.contains("eol=lf")))
    }

    pub fn add_eol_rule(&self, dir: &str) -> Result<()> {
        if !self.needs_eol_rule(dir)? {
            return Ok(());
        }

        let path = self.root().join(".gitattributes");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("{dir}/** eol=lf\n"));
        std::fs::write(&path, text)?;
        Ok(())
    }
}
