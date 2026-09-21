use crate::{GitCommandError, GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub type CommandSink = Arc<dyn Fn(GitOutput) + Send + Sync>;

/// Cleared, not overridden — `GIT_DIR` and friends override `current_dir`, and unset must
/// stay unset (doc/12-risks.md, R-22).
const INHERITED_GIT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_INDEX_VERSION",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_PREFIX",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
];

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitOutput {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u32,
}

impl RepoHandle {
    pub fn run_git(&self, args: &[&str]) -> Result<GitOutput> {
        self.spawn(args, false)
    }

    /// `GIT_OPTIONAL_LOCKS=0` keeps a background read off `index.lock`.
    pub fn run_git_reading(&self, args: &[&str]) -> Result<GitOutput> {
        self.spawn(args, true)
    }

    pub(crate) fn base_git(&self, args: &[&str]) -> Command {
        let mut command = base_command(self.root(), false);
        command.args(args);
        command
    }

    pub(crate) fn journal_entry(&self, entry: GitOutput) {
        if let Some(sink) = self.journal() {
            sink(entry);
        }
    }

    /// For the handful of commands that need a variable `base_command` deliberately pins.
    pub(crate) fn run_git_with_env(
        &self,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<GitOutput> {
        self.spawn_with(args, false, env)
    }

    fn spawn(&self, args: &[&str], reading: bool) -> Result<GitOutput> {
        self.spawn_with(args, reading, &[])
    }

    fn spawn_with(&self, args: &[&str], reading: bool, env: &[(&str, &str)]) -> Result<GitOutput> {
        let command = redact_command(args);
        let started = std::time::Instant::now();

        tracing::info!(command = %command, "running git");
        let mut process = base_command(self.root(), reading);
        for (key, value) in env {
            process.env(key, value);
        }
        let output = process.args(args).output()?;

        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let result = GitOutput {
            command,
            exit_code: output.status.code(),
            // Normalised once, here: the journal, the log file and the error window
            // all read this record, and none of them may show a credential (INV-05).
            stdout: GitCommandError::cap_stream(crate::output_text::normalise(
                &String::from_utf8_lossy(&output.stdout),
            )),
            stderr: GitCommandError::cap_stream(crate::output_text::normalise(
                &String::from_utf8_lossy(&output.stderr),
            )),
            duration_ms,
        };

        self.journal_entry(result.clone());

        if output.status.success() {
            tracing::debug!(
                command = %result.command,
                duration_ms,
                stdout = %result.stdout,
                stderr = %result.stderr,
                "git finished"
            );
            return Ok(result);
        }

        tracing::error!(
            command = %result.command,
            exit_code = ?result.exit_code,
            duration_ms,
            stderr = %result.stderr,
            "git failed"
        );
        Err(GitError::Command(GitCommandError {
            command: result.command,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        }))
    }
}

/// Without it every `git` call flashes a console window and pays for it (R-24).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn base_command(root: &Path, reading: bool) -> Command {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("LC_ALL", "C");
    // `--continue` opens an editor, and with no terminal it hangs forever (R-26).
    command.env("GIT_EDITOR", "true");
    command.env("GIT_SEQUENCE_EDITOR", "true");
    if reading {
        command.env("GIT_OPTIONAL_LOCKS", "0");
    }
    for variable in INHERITED_GIT_VARS {
        command.env_remove(variable);
    }
    command
}

const HIDDEN: &str = "<redacted>";

/// This string reaches the journal, the log file and the error dialog (INV-05).
pub fn redact_command(args: &[&str]) -> String {
    let parts: Vec<String> = args.iter().map(|arg| redact_arg(arg)).collect();
    format!("git {}", parts.join(" "))
}

fn redact_arg(arg: &str) -> String {
    if let Some((key, _)) = arg.split_once('=')
        && key.eq_ignore_ascii_case("http.extraheader")
    {
        return format!("{key}={HIDDEN}");
    }
    redact_url(arg)
}

/// Only `scheme://user:secret@host` counts: a refspec and an SSH path also carry colons.
fn redact_url(arg: &str) -> String {
    let Some((scheme, rest)) = arg.split_once("://") else {
        return arg.to_owned();
    };
    let Some((authority, tail)) = rest.split_once('@') else {
        return arg.to_owned();
    };
    let Some((user, _)) = authority.split_once(':') else {
        return arg.to_owned();
    };
    format!("{scheme}://{user}:{HIDDEN}@{tail}")
}
