use crate::{AppState, RepoId};
use git_engine::{BlameReport, FileRevision, GitError, OriginQuery, OriginReport};

/// Enough for any file a person investigates; the Navigation log streams it in chunks.
pub const FILE_LOG_LIMIT: usize = 10_000;

impl AppState {
    pub fn file_log(
        &self,
        repo: RepoId,
        path: &str,
        rev: Option<&str>,
        follow: bool,
    ) -> Result<Vec<FileRevision>, GitError> {
        self.handle(repo)?
            .file_log(path, rev, follow, FILE_LOG_LIMIT)
    }

    pub fn blame_origins(
        &self,
        repo: RepoId,
        path: &str,
        rev: Option<&str>,
        ignore_whitespace: bool,
    ) -> Result<BlameReport, GitError> {
        self.handle(repo)?
            .blame_origins(path, rev, ignore_whitespace)
    }

    pub fn origin_candidates(
        &self,
        repo: RepoId,
        query: &OriginQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<OriginReport>, GitError> {
        self.handle(repo)?.origin_candidates(query, cancelled)
    }
}
