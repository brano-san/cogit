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
        if crate::runner::fits_command_line(commits) {
            args.extend(commits.iter().map(String::as_str));
            return self.run_git(&args).map(drop);
        }
        // Past a Windows command line git would never start; `--stdin` keeps their order.
        args.push("--stdin");
        self.run_git_fed(&args, commits.join("\n").as_bytes())
            .map(drop)
    }
}
