use crate::{GitError, RepoHandle, RepoState, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RebaseOptions {
    pub onto: String,
    pub autostash: bool,
    #[serde(default)]
    pub upstream: Option<String>,
}

impl RepoHandle {
    pub fn abort_operation(&self) -> Result<()> {
        match self.operation()? {
            "bisect" => self.run_git(&["bisect", "reset"]).map(drop),
            operation => self.run_git(&[operation, "--abort"]).map(drop),
        }
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
        let onto_flag = format!("--onto={onto}");
        match options.upstream.as_deref().map(str::trim) {
            Some("") => {
                return Err(GitError::InvalidState(
                    "no commit to rebase from".to_owned(),
                ));
            }
            Some(upstream) => {
                self.check_linear_since(upstream)?;
                args.extend([onto_flag.as_str(), "--end-of-options", upstream]);
            }
            None => args.extend(["--end-of-options", onto]),
        }
        self.run_git(&args).map(drop)
    }

    /// `rebase --onto X <upstream>` replays all of `upstream..HEAD` flattened: a merge in it
    /// is dropped, and an upstream off HEAD's first-parent line drags in mainline commits.
    fn check_linear_since(&self, upstream: &str) -> Result<()> {
        let base = self.resolve_commit(upstream)?.to_string();
        if !self.is_ancestor(&base, "HEAD")? {
            return Err(GitError::InvalidState(format!(
                "{upstream} is not on the checked-out branch: the commits above it are not a line to move"
            )));
        }
        let merges =
            self.read_git(&["rev-list", "--merges", "--count", &format!("{base}..HEAD")])?;
        if merges.trim() != "0" {
            return Err(GitError::InvalidState(format!(
                "a merge commit comes after {upstream}: moving the commits above it would flatten the merge"
            )));
        }
        Ok(())
    }

    /// Only a rebase and a cherry-pick can skip; a merge has nothing to skip past.
    pub fn skip_operation(&self) -> Result<()> {
        let operation = self.operation()?;
        if !matches!(operation, "rebase" | "cherry-pick" | "revert" | "am") {
            return Err(GitError::InvalidState(format!(
                "{operation} cannot skip a step"
            )));
        }
        self.run_git(&[operation, "--skip"]).map(drop)
    }

    pub fn continue_operation(&self) -> Result<()> {
        match self.operation()? {
            "bisect" => Err(GitError::InvalidState(
                "a bisect goes on with good, bad or skip, not continue".to_owned(),
            )),
            operation => self.run_git(&[operation, "--continue"]).map(drop),
        }
    }

    /// The wrong command leaves the repository half-resolved.
    fn operation(&self) -> Result<&'static str> {
        match self.state()? {
            RepoState::Merging => Ok("merge"),
            RepoState::Rebasing => Ok("rebase"),
            RepoState::CherryPicking => Ok("cherry-pick"),
            RepoState::Reverting => Ok("revert"),
            RepoState::Bisecting { .. } => Ok("bisect"),
            RepoState::ApplyingPatches => Ok("am"),
            other => Err(GitError::InvalidState(format!(
                "nothing is in progress ({other:?})"
            ))),
        }
    }
}
