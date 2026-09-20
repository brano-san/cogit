use crate::{GitCommandError, GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Receives every command, successful or not, for the Output panel.
pub type CommandSink = Arc<dyn Fn(GitOutput) + Send + Sync>;

/// `GIT_DIR` and friends override `current_dir`, so an inherited value would send the
/// command into a different repository. Cleared, not overridden: unset must stay unset.
/// See `doc/03-git-semantics.md` section 3 and `doc/12-risks.md` (R-22).
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
    "GIT_EDITOR",
    "GIT_SEQUENCE_EDITOR",
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

    /// For commands that only read: `GIT_OPTIONAL_LOCKS=0` keeps them off `index.lock`, so
    /// a background refresh cannot collide with something the user started.
    pub fn run_git_reading(&self, args: &[&str]) -> Result<GitOutput> {
        self.spawn(args, true)
    }

    fn spawn(&self, args: &[&str], reading: bool) -> Result<GitOutput> {
        let command = format!("git {}", args.join(" "));
        let started = std::time::Instant::now();

        tracing::info!(command = %command, "running git");
        let output = base_command(self.root(), reading).args(args).output()?;

        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let result = GitOutput {
            command,
            exit_code: output.status.code(),
            stdout: GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ),
            stderr: GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ),
            duration_ms,
        };

        if let Some(sink) = self.journal() {
            sink(result.clone());
        }

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

fn base_command(root: &Path, reading: bool) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("LC_ALL", "C");
    if reading {
        command.env("GIT_OPTIONAL_LOCKS", "0");
    }
    for variable in INHERITED_GIT_VARS {
        command.env_remove(variable);
    }
    command
}
