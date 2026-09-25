use crate::{GitError, RepoHandle, Result, StashEntry};

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
        let newer = newer_than(&entries, index);

        restack(message, target.oid.as_str(), &newer, true, |args| {
            self.run_git(args).map(|_| ())
        })
    }

    /// Lists a dropped stash again at the place it had: `stash store` only ever adds on
    /// top, so the entries above that place come off first and go back over it (R-431).
    pub fn restore_stash(&self, dropped: &StashEntry) -> Result<()> {
        let entries = self.stashes()?;
        let newer = newer_than(&entries, dropped.index);
        restack(&dropped.message, &dropped.oid, &newer, false, |args| {
            self.run_git(args).map(|_| ())
        })
    }
}

/// Every entry above `index`, `stash@{0}` first, as (message, oid).
fn newer_than(entries: &[StashEntry], index: u32) -> Vec<(&str, &str)> {
    let mut newer: Vec<_> = entries.iter().filter(|entry| entry.index < index).collect();
    newer.sort_by_key(|entry| entry.index);
    newer
        .iter()
        .map(|entry| (entry.message.as_str(), entry.oid.as_str()))
        .collect()
}

/// `newer` is every entry above the target; `listed` says whether the target itself is in
/// the list and comes off too. A failure part way still stores back whatever was dropped:
/// an entry left out of the stash list is one the user can no longer see, so each one that
/// cannot go back is logged with its oid.
fn restack(
    message: &str,
    target: &str,
    newer: &[(&str, &str)],
    listed: bool,
    mut git: impl FnMut(&[&str]) -> Result<()>,
) -> Result<()> {
    let mut failure = None;
    let mut dropped = 0;
    for _ in 0..newer.len() + usize::from(listed) {
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

    use super::restack;
    use crate::GitError;

    fn run_listed(listed: bool, fail_on: usize) -> (Vec<String>, bool) {
        let mut calls = Vec::new();
        let result = restack(
            "renamed",
            "t",
            &[("zero", "a"), ("one", "b")],
            listed,
            |args| {
                calls.push(args.join(" "));
                if calls.len() == fail_on {
                    Err(GitError::InvalidState("refs/stash.lock exists".to_owned()))
                } else {
                    Ok(())
                }
            },
        );
        (calls, result.is_ok())
    }

    fn run(fail_on: usize) -> (Vec<String>, bool) {
        run_listed(true, fail_on)
    }

    #[test]
    fn a_restore_drops_only_the_newer_ones_and_puts_the_entry_under_them() {
        let (calls, ok) = run_listed(false, 0);
        assert!(ok);
        assert_eq!(
            calls,
            [
                "stash drop stash@{0}",
                "stash drop stash@{0}",
                "stash store --message renamed t",
                "stash store --message one b",
                "stash store --message zero a",
            ]
        );
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
