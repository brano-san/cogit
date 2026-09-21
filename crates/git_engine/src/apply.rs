use crate::{GitCommandError, GitError, GitOutput, RepoHandle, Result};
use std::io::Write as _;
use std::process::Stdio;

/// What a patch is applied to. Staging never touches the file on disk; Discard only
/// touches the file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchTarget {
    /// `--cached`: the index alone, which is what Stage and Unstage change.
    Index,
    /// The working tree, which is where Discard throws the lines away.
    WorkTree,
}

impl RepoHandle {
    /// Applies a synthetic patch to the index only. The full patch text goes to the log
    /// **before** it runs: a patch that corrupts a file is unrecoverable otherwise (R-04).
    pub fn apply_patch(&self, patch: &str, reverse: bool) -> Result<()> {
        self.apply_patch_to(patch, reverse, PatchTarget::Index)
    }

    pub fn apply_patch_to(&self, patch: &str, reverse: bool, target: PatchTarget) -> Result<()> {
        if patch.trim().is_empty() {
            return Err(GitError::InvalidState("an empty patch".to_owned()));
        }

        let mut args = vec!["apply", "--unidiff-zero", "--whitespace=nowarn"];
        if target == PatchTarget::Index {
            args.insert(1, "--cached");
        }
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
        let result = GitOutput::record(
            command,
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
        );
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
