use crate::{GitError, RepoHandle, Result};

impl RepoHandle {
    /// Applies each commit as a new one on top of HEAD, oldest first.
    pub fn cherry_pick(&self, commits: &[String]) -> Result<()> {
        self.replay("cherry-pick", commits)
    }

    /// Records the inverse of each commit; history itself is left alone.
    pub fn revert(&self, commits: &[String]) -> Result<()> {
        self.replay("revert", commits)
    }

    fn replay(&self, verb: &'static str, commits: &[String]) -> Result<()> {
        if commits.is_empty() {
            return Err(GitError::InvalidState(format!("no commits to {verb}")));
        }
        let mut args = vec![verb, "--no-edit"];
        args.extend(commits.iter().map(String::as_str));
        self.run_git(&args).map(drop)
    }
}
