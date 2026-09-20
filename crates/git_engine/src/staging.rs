use crate::{FileStatus, GitError, Head, RepoHandle, Result};
use std::collections::HashSet;

impl RepoHandle {
    pub fn stage(&self, paths: &[String]) -> Result<()> {
        self.run_paths(&["add", "--all", "--"], paths)
    }

    pub fn unstage(&self, paths: &[String]) -> Result<()> {
        require_paths(paths)?;
        // `restore --staged` resolves HEAD, which does not exist before the first commit.
        if matches!(self.head()?, Head::Unborn { .. }) {
            return self.run_paths(&["rm", "--cached", "-r", "--"], paths);
        }
        self.run_paths(&["restore", "--staged", "--"], paths)
    }

    /// A tracked file comes back from the index; an untracked one is removed outright.
    pub fn discard(&self, paths: &[String]) -> Result<()> {
        require_paths(paths)?;

        let untracked: HashSet<String> = self
            .worktree_files()?
            .unstaged
            .into_iter()
            .filter(|entry| entry.status == FileStatus::Untracked)
            .map(|entry| entry.path)
            .collect();

        let (to_clean, to_restore): (Vec<String>, Vec<String>) = paths
            .iter()
            .cloned()
            .partition(|path| untracked.contains(path));

        if !to_restore.is_empty() {
            self.run_paths(&["restore", "--worktree", "--"], &to_restore)?;
        }
        if !to_clean.is_empty() {
            self.run_paths(&["clean", "-fd", "--"], &to_clean)?;
        }
        Ok(())
    }

    fn run_paths(&self, prefix: &[&str], paths: &[String]) -> Result<()> {
        require_paths(paths)?;
        let mut args: Vec<&str> = prefix.to_vec();
        args.extend(paths.iter().map(String::as_str));
        self.run_git(&args).map(drop)
    }
}

fn require_paths(paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Err(GitError::InvalidState(
            "no paths given; refusing to act on the whole repository".to_owned(),
        ));
    }
    Ok(())
}

impl RepoHandle {
    /// Only the executable bit; `git add` would stage the content change with it.
    pub fn stage_mode(&self, path: &str, executable: bool) -> Result<()> {
        let tracked = self.run_git_reading(&["ls-files", "--error-unmatch", "--", path]);
        if tracked.is_err() {
            return Err(GitError::InvalidState(format!("{path} is not tracked")));
        }

        // `--chmod` re-reads the file, so re-register with the blob already indexed.
        let staged = self
            .run_git_reading(&["ls-files", "--stage", "--", path])?
            .stdout;
        let oid = staged
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| GitError::InvalidState(format!("{path} has no entry in the index")))?;

        let mode = if executable { "100755" } else { "100644" };
        let entry = format!("{mode},{oid},{path}");
        self.run_git(&["update-index", "--cacheinfo", &entry])
            .map(drop)
    }
}
