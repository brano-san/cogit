use crate::{AppState, Recovery, RepoId};
use git_engine::{BisectMark, GitError};

impl AppState {
    pub(crate) fn local_bisect_start(
        &self,
        repo: RepoId,
        bad: &str,
        good: Option<&str>,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.bisect_start(bad, good)
    }

    pub(crate) fn local_bisect_mark(
        &self,
        repo: RepoId,
        mark: BisectMark,
        rev: Option<&str>,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.bisect_mark(mark, rev)
    }

    /// In the journal as `Abort` is: the marks are gone, and Undo cannot bring them back.
    pub(crate) fn local_bisect_reset(&self, repo: RepoId) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.bisect_reset()?;
        self.record(repo, "Reset the bisect".to_owned(), Recovery::None);
        Ok(())
    }
}
