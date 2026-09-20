use crate::{GitCommandError, GitError, GitOutput, RepoHandle, Result};
use std::io::Write as _;
use std::process::Stdio;

impl RepoHandle {
    /// Applies a synthetic patch to the index only. The full patch text goes to the log
    /// **before** it runs: a patch that corrupts a file is unrecoverable otherwise (R-04).
    pub fn apply_patch(&self, patch: &str, reverse: bool) -> Result<()> {
        if patch.trim().is_empty() {
            return Err(GitError::InvalidState("an empty patch".to_owned()));
        }

        let mut args = vec!["apply", "--cached", "--unidiff-zero", "--whitespace=nowarn"];
        if reverse {
            args.push("--reverse");
        }
        args.push("-");

        let command = format!("git {}", args.join(" "));
        tracing::info!(command = %command, patch = %patch, "applying a patch");

        let started = std::time::Instant::now();
        let mut child = self
            .base_git(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(patch.as_bytes())?;
        }
        drop(child.stdin.take());

        let output = child.wait_with_output()?;
        let result = GitOutput {
            command,
            exit_code: output.status.code(),
            stdout: GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ),
            stderr: GitCommandError::cap_stream(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ),
            duration_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
        };
        self.journal_entry(result.clone());

        if output.status.success() {
            return Ok(());
        }
        tracing::error!(patch = %patch, stderr = %result.stderr, "the patch did not apply");
        Err(GitError::Command(GitCommandError {
            command: result.command,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        }))
    }
}
