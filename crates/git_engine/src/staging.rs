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

    /// Throws away work, so the two kinds of path are separated rather than guessed at:
    /// a tracked file comes back from the index, an untracked one is removed outright.
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
