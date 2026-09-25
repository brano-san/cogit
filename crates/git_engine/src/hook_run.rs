//! Running a hook alone, with no commit behind it, and the user's own check command:
//! both through the shell Git for Windows ships (T10.3, T11.3).

use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::Path;

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

        // One file per run: two dry runs at once must not delete each other's message.
        static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let scratch = self
            .git_dir()
            .join(format!("COGIT_HOOK_MSG-{}-{run}", std::process::id()));
        let args = sample_args(name, &scratch)?;

        let started = std::time::Instant::now();
        let output = hook_command(&path, self.root(), &args)
            .and_then(|mut command| crate::children::output(&mut command));
        let duration_ms = crate::runner::elapsed_ms(started);
        let _ = std::fs::remove_file(&scratch);
        let output = output?;

        let run = HookRun {
            name: name.to_owned(),
            exit_code: output.status.code(),
            stdout: crate::output_text::shown(&String::from_utf8_lossy(&output.stdout)),
            stderr: crate::output_text::shown(&String::from_utf8_lossy(&output.stderr)),
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
fn hook_command(
    path: &Path,
    root: &Path,
    args: &[String],
) -> std::io::Result<std::process::Command> {
    let mut command = bash(root)?;
    command.arg(path);
    command.args(args);
    Ok(command)
}

/// Git's bash, in the repository, with no console window and no password prompt.
#[cfg(windows)]
fn bash(root: &Path) -> std::io::Result<std::process::Command> {
    use std::os::windows::process::CommandExt as _;
    let mut command = std::process::Command::new(git_bash()?);
    command.creation_flags(crate::runner::CREATE_NO_WINDOW);
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    crate::runner::clear_inherited_git_vars(&mut command);
    Ok(command)
}

/// The bash beside the git that runs everything else. Not `bash` by name: Windows looks in
/// System32 before PATH, and Git for Windows puts only `Git\cmd` on PATH, so the name finds
/// WSL's bash (System32 or WindowsApps) or nothing.
#[cfg(windows)]
fn git_bash() -> std::io::Result<std::path::PathBuf> {
    static FOUND: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    if let Some(found) = FOUND.get() {
        return Ok(found.clone());
    }
    let exec_path = crate::runner::bare_git(&["--exec-path"])
        .map_err(|err| std::io::Error::other(err.to_string()))?
        .stdout;
    let found = bash_near(Path::new(exec_path.trim())).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no bash.exe of Git for Windows above {}", exec_path.trim()),
        )
    })?;
    Ok(FOUND.get_or_init(|| found).clone())
}

/// `<Git>/mingw64/libexec/git-core` (or `mingw32`, `clangarm64`) → `<Git>/bin/bash.exe`.
#[cfg(windows)]
fn bash_near(exec_path: &Path) -> Option<std::path::PathBuf> {
    exec_path
        .ancestors()
        .skip(1)
        .take(4)
        .flat_map(|dir| {
            [
                dir.join("bin").join("bash.exe"),
                dir.join("usr").join("bin").join("bash.exe"),
            ]
        })
        .find(|candidate| candidate.is_file())
}

#[cfg(not(windows))]
fn hook_command(
    path: &Path,
    root: &Path,
    args: &[String],
) -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(path);
    command.args(args);
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    crate::runner::clear_inherited_git_vars(&mut command);
    Ok(command)
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
            .and_then(|mut command| crate::children::output(&mut command))
            .map_err(|err| GitError::Io(format!("cannot run the check: {err}")))?;
        let duration_ms = crate::runner::elapsed_ms(started);

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
                self.root(),
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
fn shell_command(command: &str, root: &Path) -> std::io::Result<std::process::Command> {
    let mut spawned = bash(root)?;
    spawned.args(["-lc", command]);
    Ok(spawned)
}

#[cfg(not(windows))]
fn shell_command(command: &str, root: &Path) -> std::io::Result<std::process::Command> {
    let mut spawned = std::process::Command::new("sh");
    spawned.args(["-c", command]);
    spawned.current_dir(root);
    spawned.env("GIT_TERMINAL_PROMPT", "0");
    crate::runner::clear_inherited_git_vars(&mut spawned);
    Ok(spawned)
}
