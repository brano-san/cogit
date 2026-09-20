use crate::{GitError, RepoHandle, Result};

impl RepoHandle {
    pub fn cherry_pick(&self, commits: &[String]) -> Result<()> {
        self.replay("cherry-pick", commits)
    }

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
