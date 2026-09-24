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
        let mut newer: Vec<_> = entries.iter().filter(|entry| entry.index < index).collect();
        newer.sort_by_key(|entry| entry.index);
        let newer: Vec<(&str, &str)> = newer
            .iter()
            .map(|entry| (entry.message.as_str(), entry.oid.as_str()))
            .collect();

        rename_with(message, target.oid.as_str(), &newer, |args| {
            self.run_git(args).map(|_| ())
        })
    }
}

/// `newer` is every entry above the target, `stash@{0}` first, as (message, oid). A failure
/// part way still stores back whatever was dropped: an entry left out of the stash list is
/// one the user can no longer see, so each one that cannot go back is logged with its oid.
fn rename_with(
    message: &str,
    target: &str,
    newer: &[(&str, &str)],
    mut git: impl FnMut(&[&str]) -> Result<()>,
) -> Result<()> {
    let mut failure = None;
    let mut dropped = 0;
    for _ in 0..=newer.len() {
        if let Err(err) = git(&["stash", "drop", "stash@{0}"]) {
            failure = Some(err);
            break;
        }
        dropped += 1;
    }

    let restore: Vec<(&str, &str)> = if failure.is_none() {
        std::iter::once((message, target))
            .chain(newer.iter().rev().copied())
            .collect()
    } else {
        // The target is still there under its old name; only the newer ones went.
        newer[..dropped].iter().rev().copied().collect()
    };
    for (text, oid) in restore {
        if let Err(err) = git(&["stash", "store", "--message", text, oid]) {
            tracing::error!(error = ?err, %oid, context = "stash rename could not store an entry back");
            failure.get_or_insert(err);
        }
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::rename_with;
    use crate::GitError;

    fn run(fail_on: usize) -> (Vec<String>, bool) {
        let mut calls = Vec::new();
        let result = rename_with("renamed", "t", &[("zero", "a"), ("one", "b")], |args| {
            calls.push(args.join(" "));
            if calls.len() == fail_on {
                Err(GitError::InvalidState("refs/stash.lock exists".to_owned()))
            } else {
                Ok(())
            }
        });
        (calls, result.is_ok())
    }

    #[test]
    fn a_rename_drops_down_to_the_target_and_stores_them_back_in_order() {
        let (calls, ok) = run(0);
        assert!(ok);
        assert_eq!(
            calls,
            [
                "stash drop stash@{0}",
                "stash drop stash@{0}",
                "stash drop stash@{0}",
                "stash store --message renamed t",
                "stash store --message one b",
                "stash store --message zero a",
            ]
        );
    }

    // Something else took the stash lock between two drops: the first entry was already
    // gone and nothing put it back.
    #[test]
    fn a_drop_that_fails_part_way_stores_back_what_went() {
        let (calls, ok) = run(2);
        assert!(!ok);
        assert_eq!(
            calls,
            [
                "stash drop stash@{0}",
                "stash drop stash@{0}",
                "stash store --message zero a",
            ]
        );
    }

    #[test]
    fn a_store_that_fails_does_not_leave_the_rest_out() {
        let (calls, ok) = run(4);
        assert!(!ok);
        assert_eq!(calls.len(), 6, "every entry was offered back: {calls:?}");
    }
}
