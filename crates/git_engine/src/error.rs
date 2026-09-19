//! Error types.
//!
//! The central rule (INV-05): when the `git` CLI fails, the user sees exactly what Git
//! said. `stderr` carries the pull-request link, the pre-receive hook message and the
//! reason a push was rejected — replacing it with "Push failed" destroys the only
//! information worth having.

use serde::Serialize;

/// Streams are capped so a runaway command cannot exhaust memory. When the cap is hit,
/// the truncation is stated explicitly rather than hidden.
pub const MAX_STREAM_BYTES: usize = 1024 * 1024;

/// A `git` CLI invocation that exited with a non-zero status.
///
/// Every field is carried to the UI verbatim.
#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[error("Command `{command}` failed (exit code {exit_code:?})")]
pub struct GitCommandError {
    /// The full command line as it would have been typed, for the "Copy Output" action.
    pub command: String,
    /// `None` when the process was terminated by a signal instead of exiting.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitCommandError {
    /// Truncates a captured stream to [`MAX_STREAM_BYTES`], marking the cut explicitly.
    ///
    /// Truncation is visible to the user by design: a silently shortened error message
    /// is worse than a long one.
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

/// Everything that can go wrong while talking to a repository.
///
/// Variants are distinct so the UI can react differently: a CLI failure opens the
/// Git Error Dialog with raw output, everything else becomes a toast.
#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "data")]
pub enum GitError {
    #[error(transparent)]
    Command(#[from] GitCommandError),

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
        // A multi-byte character straddling the cap must not produce invalid UTF-8.
        let input = "я".repeat(MAX_STREAM_BYTES);
        let capped = GitCommandError::cap_stream(input);
        assert!(capped.is_char_boundary(capped.len()));
    }

    #[test]
    fn error_display_names_the_command() {
        let err = GitCommandError {
            command: "git push origin main".to_owned(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "protected branch hook declined".to_owned(),
        };
        assert!(err.to_string().contains("git push origin main"));
    }
}
