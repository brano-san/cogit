use crate::{RepoHandle, Result};

impl RepoHandle {
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

    /// Through `gix`: a spawned `rev-parse` costs tens of milliseconds (R-24).
    fn stash_top(&self) -> Option<String> {
        self.repo
            .find_reference("refs/stash")
            .ok()?
            .peel_to_id()
            .ok()
            .map(|id| id.detach().to_string())
    }
}
