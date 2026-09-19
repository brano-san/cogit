//! INV-05: when the `git` CLI fails the user sees exactly what Git said — `stderr`
//! carries the pre-receive message and the reason a push was rejected.

use serde::Serialize;

pub const MAX_STREAM_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[error("Command `{command}` failed (exit code {exit_code:?})")]
#[serde(rename_all = "camelCase")]
pub struct GitCommandError {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitCommandError {
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

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
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
