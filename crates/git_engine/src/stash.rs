use crate::{RepoHandle, Result};

impl RepoHandle {
    /// Stashes exactly the given paths and returns the stash commit, or `None` when there
    /// was nothing to put away. Untracked files are included: discarding one destroys it
    /// just as thoroughly as discarding an edit.
    pub fn stash_paths(&self, paths: &[String], message: &str) -> Result<Option<String>> {
        let before = self.stash_top();

        let mut args = vec![
            "stash",
            "push",
            "--include-untracked",
            "--message",
            message,
            "--",
        ];
        args.extend(paths.iter().map(String::as_str));
        self.run_git(&args)?;

        let after = self.stash_top();
        Ok(if after == before { None } else { after })
    }

    pub fn stash_apply(&self, oid: &str) -> Result<()> {
        self.run_git(&["stash", "apply", "--index", oid])
            .or_else(|_| self.run_git(&["stash", "apply", oid]))
            .map(drop)
    }

    fn stash_top(&self) -> Option<String> {
        self.run_git_reading(&["rev-parse", "--verify", "--quiet", "refs/stash"])
            .ok()
            .map(|out| out.stdout.trim().to_owned())
            .filter(|oid| !oid.is_empty())
    }
}
