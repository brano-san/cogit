use crate::{CommitRow, GitError, RepoHandle, Result};
use serde::Serialize;
use std::collections::HashSet;

/// Every commit the graph tips reach, and the tips it was counted from. Kept between calls
/// by the caller: after most operations the tips have not moved, or only moved forward.
#[derive(Debug, Default, Clone)]
pub struct Reachable {
    tips: Vec<gix::ObjectId>,
    set: HashSet<gix::ObjectId>,
    counted: bool,
    recounts: u32,
}

impl Reachable {
    /// How many times the whole history was walked, rather than just its new top.
    #[must_use]
    pub fn recounts(&self) -> u32 {
        self.recounts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    pub selector: String,
    pub oid: String,
    pub action: String,
    pub message: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

impl RepoHandle {
    /// Newest first, like `git reflog`.
    pub fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        if self.head().is_err() || matches!(self.head()?, crate::Head::Unborn { .. }) {
            return Ok(Vec::new());
        }

        let count = limit.to_string();
        let out = self.run_git_reading(&[
            "reflog",
            "--max-count",
            &count,
            "--format=%H%x00%gd%x00%gs%x00%at",
        ])?;

        let mut entries = Vec::new();
        for line in out.stdout.lines().filter(|l| !l.is_empty()) {
            let mut parts = line.split('\0');
            let (Some(oid), Some(selector), Some(subject), Some(timestamp)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                tracing::warn!(line, "skipping an unreadable reflog entry");
                continue;
            };
            let (action, message) = subject.split_once(": ").unwrap_or((subject, ""));
            entries.push(ReflogEntry {
                selector: selector.to_owned(),
                oid: oid.to_owned(),
                action: action.trim().to_owned(),
                message: message.trim().to_owned(),
                timestamp: timestamp.trim().parse().unwrap_or_default(),
            });
        }
        Ok(entries)
    }

    /// Commits the reflog remembers but no ref can reach — what a reset or a rebase left
    /// behind. This is the only way back to them from inside the client.
    pub fn lost_commits(&self, limit: usize) -> Result<Vec<CommitRow>> {
        self.lost_commits_with(limit, &mut Reachable::default())
    }

    /// As `lost_commits`, reusing what `cache` knows about reachability.
    pub fn lost_commits_with(&self, limit: usize, cache: &mut Reachable) -> Result<Vec<CommitRow>> {
        self.update_reachable(cache)?;
        let mut seen = HashSet::new();
        let mut lost = Vec::new();

        for entry in self.reflog(limit)? {
            let known = gix::ObjectId::from_hex(entry.oid.as_bytes())
                .is_ok_and(|id| cache.set.contains(&id));
            if known || !seen.insert(entry.oid.clone()) {
                continue;
            }
            if let Ok(details) = self.commit_details(&entry.oid) {
                lost.push(CommitRow {
                    oid: details.oid,
                    parents: details.parents,
                    summary: details.summary,
                    author_name: details.author.name,
                    author_email: details.author.email,
                    timestamp: details.author.timestamp,
                    tz_offset_minutes: details.author.tz_offset_minutes,
                });
            }
        }
        Ok(lost)
    }

    /// Unmoved tips cost nothing; tips that only moved forward cost the walk down to the
    /// commits already known. A tip that vanished unseen can shrink the set: recount.
    fn update_reachable(&self, cache: &mut Reachable) -> Result<()> {
        let tips = self.graph_tips()?;
        if cache.counted && cache.tips == tips {
            return Ok(());
        }
        if cache.counted {
            let mut met = HashSet::new();
            let mut added = HashSet::new();
            let mut stack: Vec<gix::ObjectId> = tips.clone();
            while let Some(id) = stack.pop() {
                if cache.set.contains(&id) {
                    met.insert(id);
                    continue;
                }
                if !added.insert(id) {
                    continue;
                }
                stack.extend(self.parents_of(id)?);
            }
            if cache
                .tips
                .iter()
                .all(|old| tips.contains(old) || met.contains(old))
            {
                cache.set.extend(added);
                cache.tips = tips;
                return Ok(());
            }
        }
        cache.set = self.reachable_from(&tips)?;
        cache.tips = tips;
        cache.counted = true;
        cache.recounts += 1;
        Ok(())
    }

    fn parents_of(&self, id: gix::ObjectId) -> Result<Vec<gix::ObjectId>> {
        let commit = self
            .repo
            .find_commit(id)
            .map_err(|err| GitError::Internal(format!("cannot read commit: {err}")))?;
        Ok(commit.parent_ids().map(gix::Id::detach).collect())
    }

    /// Ids only: nothing here needs a message or an author decoded.
    fn reachable_from(&self, tips: &[gix::ObjectId]) -> Result<HashSet<gix::ObjectId>> {
        let walk = self
            .repo
            .rev_walk(tips.iter().copied())
            .all()
            .map_err(|err| GitError::Internal(format!("cannot walk history: {err}")))?;
        let mut set = HashSet::new();
        for step in walk {
            match step {
                Ok(info) => {
                    set.insert(info.id);
                }
                Err(err) => tracing::warn!(error = %err, "skipping an unreadable commit"),
            }
        }
        Ok(set)
    }
}
