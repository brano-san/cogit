use crate::{GitError, RepoHandle, RepoState, Result};

impl RepoHandle {
    pub fn abort_operation(&self) -> Result<()> {
        self.run_git(&[self.operation()?, "--abort"]).map(drop)
    }

    pub fn continue_operation(&self) -> Result<()> {
        self.run_git(&[self.operation()?, "--continue"]).map(drop)
    }

    /// The wrong command leaves the repository half-resolved.
    fn operation(&self) -> Result<&'static str> {
        match self.state()? {
            RepoState::Merging => Ok("merge"),
            RepoState::Rebasing => Ok("rebase"),
            RepoState::CherryPicking => Ok("cherry-pick"),
            RepoState::Reverting => Ok("revert"),
            RepoState::Bisecting => Ok("bisect"),
            other => Err(GitError::InvalidState(format!(
                "nothing is in progress ({other:?})"
            ))),
        }
    }
}
