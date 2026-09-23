use crate::{GitError, RepoHandle, Result};

impl RepoHandle {
    /// Drop and store back every entry down to `index`, so none moves (R-252).
    pub fn rename_stash(&self, index: u32, message: &str) -> Result<()> {
        let message = message.trim();
        if message.is_empty() {
            return Err(GitError::InvalidState("an empty stash message".to_owned()));
        }
        let entries = self.stashes()?;
        let target = entries
            .iter()
            .find(|entry| entry.index == index)
            .ok_or_else(|| GitError::InvalidState(format!("no stash at index {index}")))?;
        let newer: Vec<_> = entries.iter().filter(|entry| entry.index < index).collect();

        for _ in 0..=index {
            self.run_git(&["stash", "drop", "stash@{0}"])?;
        }
        let restore = std::iter::once((message, target.oid.as_str())).chain(
            newer
                .iter()
                .rev()
                .map(|entry| (entry.message.as_str(), entry.oid.as_str())),
        );
        for (text, oid) in restore {
            if let Err(err) = self.run_git(&["stash", "store", "--message", text, oid]) {
                tracing::error!(error = ?err, %oid, context = "stash rename could not store an entry back");
                return Err(err);
            }
        }
        Ok(())
    }
}
