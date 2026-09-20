use crate::{GitError, RepoHandle, RepoState, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RebaseOptions {
    pub onto: String,
    pub autostash: bool,
}

impl RepoHandle {
    pub fn abort_operation(&self) -> Result<()> {
        self.run_git(&[self.operation()?, "--abort"]).map(drop)
    }

    pub fn rebase(&self, options: &RebaseOptions) -> Result<()> {
        let onto = options.onto.trim();
        if onto.is_empty() {
            return Err(GitError::InvalidState("nothing to rebase onto".to_owned()));
        }

        let mut args = vec!["rebase"];
        if options.autostash {
            args.push("--autostash");
        }
        args.push(onto);
        self.run_git(&args).map(drop)
    }

    /// Only a rebase and a cherry-pick can skip; a merge has nothing to skip past.
    pub fn skip_operation(&self) -> Result<()> {
        let operation = self.operation()?;
        if !matches!(operation, "rebase" | "cherry-pick" | "revert") {
            return Err(GitError::InvalidState(format!(
                "{operation} cannot skip a step"
            )));
        }
        self.run_git(&[operation, "--skip"]).map(drop)
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
