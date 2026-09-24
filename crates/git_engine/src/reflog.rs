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

pub(crate) struct ReflogLine {
    /// Its place in the reflog, counting the entries skipped here: git's `@{n}`.
    pub(crate) position: usize,
    pub(crate) oid: gix::ObjectId,
    pub(crate) message: String,
    pub(crate) author_time: i64,
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
        let lines = self.reflog_of("HEAD", limit)?;
        Ok(lines
            .into_iter()
            .map(|line| {
                let (action, message) =
                    line.message.split_once(": ").unwrap_or((&line.message, ""));
                ReflogEntry {
                    selector: format!("HEAD@{{{}}}", line.position),
                    oid: line.oid.to_string(),
                    action: action.trim().to_owned(),
                    message: message.trim().to_owned(),
                    timestamp: line.author_time,
                }
            })
            .collect())
    }

    /// A reflog newest first, the way `git log -g` walks it: the commit each entry moved
    /// to, the entry's message, the commit's author time. Read through `gix`: the `git`
    /// process it replaces cost tens of milliseconds a call on Windows (R-24, R-196).
    pub(crate) fn reflog_of(&self, name: &str, limit: usize) -> Result<Vec<ReflogLine>> {
        let Ok(reference) = self.repo.find_reference(name) else {
            return Ok(Vec::new());
        };
        let mut platform = reference.log_iter();
        let Some(lines) = platform
            .rev()
            .map_err(|err| GitError::Internal(format!("cannot read the reflog: {err}")))?
        else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (position, line) in lines.enumerate() {
            if out.len() == limit {
                break;
            }
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    tracing::warn!(error = %err, "skipping an unreadable reflog entry");
                    continue;
                }
            };
            // `git log -g` shows the commit; an entry whose commit is gone has nothing to show.
            let Ok(commit) = self.repo.find_commit(line.new_oid) else {
                continue;
            };
            let author_time = commit
                .author()
                .ok()
                .and_then(|author| author.time().ok())
                .map_or(0, |time| time.seconds);
            out.push(ReflogLine {
                position,
                oid: line.new_oid,
                message: line.message.to_string(),
                author_time,
            });
        }
        Ok(out)
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
        let tips = self.keeping_tips()?;
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

    /// What keeps a commit from being lost: the graph's tips, and every tag. The graph
    /// itself does not walk from tags, so they are added here only.
    fn keeping_tips(&self) -> Result<Vec<gix::ObjectId>> {
        let mut tips = self.graph_tips()?;
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        let tags = platform
            .tags()
            .map_err(|err| GitError::Internal(format!("cannot list tags: {err}")))?;
        for mut tag in tags.flatten() {
            // An annotated tag names a tag object; the commit is under it.
            let Ok(id) = tag.peel_to_id() else { continue };
            if self.repo.find_commit(id.detach()).is_ok() {
                tips.push(id.detach());
            }
        }
        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
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
