//! INV-05: when the `git` CLI fails the user sees exactly what Git said — `stderr`
//! carries the pre-receive message and the reason a push was rejected.

use serde::Serialize;

const MAX_STREAM_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[error("Command `{command}` failed (exit code {exit_code:?})")]
#[serde(rename_all = "camelCase")]
pub struct GitCommandError {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// What to call this in the window title. Derived from `command`, so the heading
    /// and the output below it always describe the same run (R-87).
    pub operation: String,
    /// One line over the output, never instead of it.
    pub summary: String,
}

impl GitCommandError {
    /// Built from a finished run, so the two records cannot describe different things.
    #[must_use]
    pub fn from_output(output: crate::GitOutput) -> Self {
        Self {
            operation: output.operation,
            summary: output.summary,
            command: output.command,
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }

    #[must_use]
    pub fn cap_stream(stream: String) -> String {
        if stream.len() <= MAX_STREAM_BYTES {
            return stream;
        }
        let mut cut = MAX_STREAM_BYTES;
        while cut > 0 && !stream.is_char_boundary(cut) {
            cut -= 1;
        }
        let dropped = stream.len() - cut;
        format!(
            "{}\n[... {dropped} bytes truncated by Cogit ...]",
            &stream[..cut]
        )
    }
}

impl From<GitCommandError> for GitError {
    fn from(error: GitCommandError) -> Self {
        Self::Command(Box::new(error))
    }
}

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum GitError {
    /// Boxed: it carries both streams, and an unboxed variant makes every `Result` in
    /// the crate as wide as the largest failure it could ever hold.
    #[error(transparent)]
    Command(Box<GitCommandError>),

    #[error("repository not found at {0}")]
    RepoNotFound(String),

    #[error("repository is busy: {0}")]
    RepoBusy(String),

    #[error("invalid repository state: {0}")]
    InvalidState(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for GitError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_streams_pass_through_untouched() {
        let s = "fatal: not a git repository".to_owned();
        assert_eq!(GitCommandError::cap_stream(s.clone()), s);
    }

    #[test]
    fn long_streams_are_truncated_visibly() {
        let capped = GitCommandError::cap_stream("x".repeat(MAX_STREAM_BYTES + 500));
        assert!(capped.contains("truncated by Cogit"));
        assert!(capped.len() < MAX_STREAM_BYTES + 200);
    }

    #[test]
    fn truncation_never_splits_a_character() {
        let input = "я".repeat(MAX_STREAM_BYTES);
        let capped = GitCommandError::cap_stream(input);
        assert!(capped.is_char_boundary(capped.len()));
    }

    #[test]
    fn error_display_names_the_command() {
        let err = GitCommandError::from_output(crate::GitOutput::record(
            "git push origin main".to_owned(),
            Some(1),
            "",
            "protected branch hook declined",
            0,
        ));
        assert!(err.to_string().contains("git push origin main"));
    }

    /// R-87: the heading and the output below it must describe the same run.
    #[test]
    fn the_operation_is_the_command_that_actually_ran() {
        let err = GitCommandError::from_output(crate::GitOutput::record(
            "git push origin main".to_owned(),
            Some(1),
            "",
            "error: failed to push some refs",
            0,
        ));
        assert_eq!(err.operation, "Push");
        assert_eq!(err.summary, "error: failed to push some refs");
    }
}
