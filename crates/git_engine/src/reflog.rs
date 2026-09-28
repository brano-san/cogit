use crate::{CommitRow, GitError, RepoHandle, Result};
use rayon::prelude::*;
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
    /// Every object id already looked at, and the commits among them: a new call reads the
    /// type of the ids it has not seen, not of all of them (R-622).
    seen: HashSet<gix::ObjectId>,
    commits: Vec<gix::ObjectId>,
    listings: u32,
    /// Set only when every timestamp it hashes is old enough to change on a write.
    stamp: Option<u64>,
}

impl Reachable {
    /// How many times the whole history was walked, rather than just its new top.
    #[must_use]
    pub fn recounts(&self) -> u32 {
        self.recounts
    }

    /// How many times the object ids were listed, rather than skipped as unchanged.
    #[must_use]
    pub fn listings(&self) -> u32 {
        self.listings
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
        self.reflog_for("HEAD", limit)
    }

    /// The reflog of a local branch, or of `HEAD`; selectors read `main@{n}`.
    pub fn reflog_for(&self, name: &str, limit: usize) -> Result<Vec<ReflogEntry>> {
        let full = if name == "HEAD" {
            name.to_owned()
        } else {
            format!("refs/heads/{name}")
        };
        let lines = self.reflog_of(&full, limit)?;
        Ok(lines
            .into_iter()
            .map(|line| {
                let (action, message) =
                    line.message.split_once(": ").unwrap_or((&line.message, ""));
                ReflogEntry {
                    selector: format!("{name}@{{{}}}", line.position),
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

    /// Every commit no ref or worktree HEAD can reach, newest first: what a reset, a
    /// rebase, a deleted branch or a dropped stash left behind. The reflog is not the
    /// source: a commit no reflog remembers is just as lost (R-622).
    pub fn lost_commits(&self, limit: usize) -> Result<Vec<CommitRow>> {
        self.lost_commits_with(limit, &mut Reachable::default())
    }

    /// As `lost_commits`, reusing what `cache` knows about reachability.
    pub fn lost_commits_with(&self, limit: usize, cache: &mut Reachable) -> Result<Vec<CommitRow>> {
        self.update_reachable(cache)?;
        let mut dated: Vec<(i64, gix::ObjectId)> = Vec::new();
        self.scan_new_commits(cache)?;
        for &id in &cache.commits {
            if cache.set.contains(&id) {
                continue;
            }
            let Ok(commit) = self.repo.find_commit(id) else {
                continue;
            };
            let time = commit
                .author()
                .ok()
                .and_then(|author| author.time().ok())
                .map_or(0, |time| time.seconds);
            dated.push((time, id));
        }
        dated.sort_unstable_by(|a, b| b.cmp(a));
        dated.truncate(limit);

        let mailmap = self.mailmap();
        let mut lost = Vec::with_capacity(dated.len());
        for (_, id) in dated {
            if let Ok(details) = self.commit_details_with(&id.to_string(), &mailmap) {
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
            // Where a shallow clone ends: the parents are not here, as the full walk knows.
            let shallow = self.shallow_commits();
            let mut met = HashSet::new();
            let mut added = HashSet::new();
            let mut stack: Vec<gix::ObjectId> = tips.clone();
            while let Some(id) = stack.pop() {
                if cache.set.contains(&id) {
                    met.insert(id);
                    continue;
                }
                if !added.insert(id) || shallow.binary_search(&id).is_ok() {
                    continue;
                }
                match self.parents_of(id) {
                    Ok(parents) => stack.extend(parents),
                    Err(err) => {
                        tracing::error!(error = ?err, context = "lost commits: skipping an unreadable commit")
                    }
                }
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

    /// What keeps a commit from being lost: every ref (branches, remotes, tags, `stash`,
    /// notes, `refs/pull/*`…) and the HEAD of every worktree, detached ones included. Reflogs
    /// do not keep a commit: they are how a lost one is found (R-622).
    fn keeping_tips(&self) -> Result<Vec<gix::ObjectId>> {
        let repo = self.repo.main_repo().unwrap_or_else(|_| self.repo.clone());
        let mut tips = Vec::new();
        let mut heads = vec![repo.clone()];
        for proxy in repo.worktrees().unwrap_or_default() {
            match proxy.into_repo_with_possibly_inaccessible_worktree() {
                Ok(linked) => heads.push(linked),
                Err(err) => {
                    tracing::error!(error = ?err, context = "lost commits: unreadable worktree")
                }
            }
        }
        heads.push(self.repo.clone());
        for head in heads {
            if let Ok(id) = head.head_id() {
                tips.push(id.detach());
            }
        }
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        for mut reference in platform
            .all()
            .map_err(|err| GitError::Internal(format!("cannot list references: {err}")))?
            .flatten()
        {
            // An annotated tag names a tag object; the commit is under it.
            let Ok(id) = reference.peel_to_id() else {
                continue;
            };
            if self.repo.find_commit(id.detach()).is_ok() {
                tips.push(id.detach());
            }
        }
        // `stash@{n}` older than the top one is held by the stash reflog alone: the Stashes
        // section lists it, so it is kept, not lost.
        for line in self.reflog_of("refs/stash", usize::MAX)? {
            tips.push(line.oid);
        }
        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
    }

    /// Commits in the object database, loose and packed alike, that `cache` has not seen.
    /// The ids are listed again only when the object directories changed: any doubt
    /// (alternates, unreadable directory, a timestamp too recent to tell) lists them.
    fn scan_new_commits(&self, cache: &mut Reachable) -> Result<()> {
        let stamp = self.object_store_stamp();
        if stamp.is_some() && stamp == cache.stamp {
            return Ok(());
        }
        cache.stamp = stamp;
        cache.listings += 1;
        let ids = self
            .repo
            .objects
            .iter()
            .map_err(|err| GitError::Internal(format!("cannot list objects: {err}")))?;
        let fresh: Vec<gix::ObjectId> = ids.flatten().filter(|id| cache.seen.insert(*id)).collect();
        let shared = self.repo.clone().into_sync();
        let found: Vec<gix::ObjectId> = fresh
            .par_chunks(2048)
            .flat_map_iter(|chunk| {
                let repo = shared.to_thread_local();
                chunk
                    .iter()
                    .filter(|id| match repo.find_header(**id) {
                        Ok(header) => header.kind() == gix::object::Kind::Commit,
                        Err(err) => {
                            tracing::error!(error = ?err, context = "lost commits: unreadable object header");
                            false
                        }
                    })
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        cache.commits.extend(found);
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

impl RepoHandle {
    /// A hash of the names and times of the `objects/xx` directories and the packs. A write
    /// changes one of them; `None` when the store cannot be vouched for.
    fn object_store_stamp(&self) -> Option<u64> {
        use std::hash::{Hash as _, Hasher as _};
        // Coarser than any filesystem's timestamps (FAT: 2 s).
        const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);
        let root = self.repo.objects.store_ref().path().to_owned();
        if root.join("info").join("alternates").exists() {
            return None;
        }
        let now = std::time::SystemTime::now();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for dir in [root.clone(), root.join("pack")] {
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(&dir).ok()? {
                let entry = entry.ok()?;
                let meta = entry.metadata().ok()?;
                let modified = meta.modified().ok()?;
                if now.duration_since(modified).is_ok_and(|age| age < SETTLE) {
                    return None;
                }
                entries.push((entry.file_name(), modified, meta.len()));
            }
            entries.sort();
            entries.hash(&mut hasher);
        }
        Some(hasher.finish())
    }
}
