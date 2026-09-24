//! INV-05: when the `git` CLI fails the user sees exactly what Git said — `stderr`
//! carries the pre-receive message and the reason a push was rejected.

use serde::Serialize;

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[error("Command `{command}` failed (exit code {exit_code:?})")]
#[serde(rename_all = "camelCase")]
pub struct GitCommandError {
    /// The journal entry this came from, so the window can offer the full record.
    pub id: u32,
    pub repo: String,
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
            id: output.id,
            repo: output.repo,
            operation: output.operation,
            summary: output.summary,
            command: output.command,
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        }
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

    #[error("invalid repository state: {0}")]
    InvalidState(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("internal error: {0}")]
    Internal(String),

    /// A submodule that cannot be opened, with the reason rather than "not a repository".
    #[error("{0}")]
    ModuleUnavailable(crate::ModuleProblem),

    /// Git refused a config file's text; nothing was written.
    #[error("{0}")]
    ConfigInvalid(crate::ConfigProblem),
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
    fn error_display_names_the_command() {
        let err = GitCommandError::from_output(crate::GitOutput::record(
            std::path::Path::new("."),
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
            std::path::Path::new("."),
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
