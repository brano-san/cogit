use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub oid: String,
    /// Git order: the first parent is the mainline, which lane allocation relies on.
    pub parents: Vec<String>,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub committer_identity: Option<String>,
    /// specta refuses `i64`; Unix seconds stay far below 2^53, so a number is exact.
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub tz_offset_minutes: i32,
}

/// The part of a row read from the commit object itself; the graph reads it for the rows
/// on screen only (R-302). A history has far fewer identities than commits: author names,
/// addresses and distinct committer identities are shared through [`Names`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitText {
    pub summary: String,
    pub author_name: std::sync::Arc<str>,
    pub author_email: std::sync::Arc<str>,
    pub committer_identity: Option<std::sync::Arc<str>>,
    pub tz_offset_minutes: i32,
}

/// Identity strings as read, one allocation each however many commits carry them.
#[derive(Debug, Default)]
pub struct Names {
    set: std::collections::HashSet<std::sync::Arc<str>>,
    bytes: usize,
}

impl Names {
    /// The shared copy of `text`, made on first sight.
    pub fn intern(&mut self, text: &str) -> std::sync::Arc<str> {
        if let Some(known) = self.set.get(text) {
            return std::sync::Arc::clone(known);
        }
        let new: std::sync::Arc<str> = std::sync::Arc::from(text);
        self.bytes += text.len() + size_of::<std::sync::Arc<str>>() + 2 * size_of::<usize>();
        self.set.insert(std::sync::Arc::clone(&new));
        new
    }

    /// `text`'s identities swapped for the shared copies.
    pub fn share(&mut self, text: &mut CommitText) {
        text.author_name = self.intern(&text.author_name);
        text.author_email = self.intern(&text.author_email);
        if let Some(identity) = &mut text.committer_identity {
            *identity = self.intern(identity);
        }
    }

    /// Roughly what it holds, for a cache that counts bytes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

const SECONDS_PER_MINUTE: i32 = 60;

pub(crate) fn distinct_committer(author: (&str, &str), committer: (&str, &str)) -> Option<String> {
    if author == committer {
        return None;
    }
    match committer {
        ("", "") => None,
        (text, "") | ("", text) => Some(text.to_owned()),
        (name, email) => Some(format!("{name} <{email}>")),
    }
}

impl RepoHandle {
    /// The commit a merge stopped on conflicts is taking in, peeled; the first of an octopus.
    #[must_use]
    pub fn merge_head(&self) -> Option<gix::ObjectId> {
        let text = std::fs::read_to_string(self.git_dir().join("MERGE_HEAD")).ok()?;
        let id = gix::ObjectId::from_hex(text.lines().next()?.trim().as_bytes()).ok()?;
        self.repo
            .find_object(id)
            .ok()?
            .peel_to_commit()
            .ok()
            .map(|commit| commit.id)
    }

    pub(crate) fn graph_tips(&self) -> Result<Vec<gix::ObjectId>> {
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;

        let mut tips = Vec::new();
        let local = platform
            .local_branches()
            .map_err(|err| GitError::Internal(format!("cannot list local branches: {err}")))?;
        collect_tips(local, &mut tips);

        let remote = platform
            .remote_branches()
            .map_err(|err| GitError::Internal(format!("cannot list remote branches: {err}")))?;
        collect_tips(remote, &mut tips);

        if let Ok(head) = self.repo.head_id() {
            tips.push(head.detach());
        }
        tips.extend(self.merge_head());

        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
    }

    /// Where the graph walk of `query` starts, hashed: every ref when all are drawn, else the
    /// commits the ticked names resolve to (so `stash drop stash@{1}`, which moves no ref, is
    /// seen) and the names left out. Always where HEAD points, and the shallow boundary:
    /// `fetch --unshallow` moves no ref. Equal prints mean the walk would start from the same
    /// tips and stop at the same commits; a fetch that moved only `origin/*` leaves a
    /// "HEAD only" graph as it was.
    pub fn refs_fingerprint(&self, query: &crate::CommitQuery) -> Result<u64> {
        use std::hash::{Hash as _, Hasher as _};
        fn add(hasher: &mut impl std::hash::Hasher, reference: &gix::Reference<'_>) {
            reference.name().as_bstr().hash(hasher);
            match reference.target() {
                gix::refs::TargetRef::Object(id) => id.as_bytes().hash(hasher),
                gix::refs::TargetRef::Symbolic(name) => name.as_bstr().hash(hasher),
            }
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if query.visible_refs.is_some() {
            let tips = self.tips_for(query)?;
            tips.ids.hash(&mut hasher);
            tips.skipped
                .iter()
                .for_each(|skipped| skipped.name.hash(&mut hasher));
        } else if let Some(stamp) = self.refs_stamp(REFS_SETTLE) {
            stamp.hash(&mut hasher);
        } else {
            let platform = self
                .repo
                .references()
                .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
            for reference in platform
                .all()
                .map_err(|err| GitError::Internal(format!("cannot list references: {err}")))?
            {
                let reference = reference
                    .map_err(|err| GitError::Internal(format!("cannot read a reference: {err}")))?;
                add(&mut hasher, &reference);
            }
        }
        if let Ok(head) = self.repo.find_reference("HEAD") {
            add(&mut hasher, &head);
        }
        // Starting a merge moves no ref, and the graph draws the merged branch then (07 §4).
        self.merge_head().hash(&mut hasher);
        self.shallow_commits().hash(&mut hasher);
        Ok(hasher.finish())
    }

    pub(crate) fn refs_stamp(&self, settle: std::time::Duration) -> Option<u64> {
        use std::hash::{Hash as _, Hasher as _};
        let store = &self.repo.refs;
        let common = store.common_dir_resolved();
        if common.join("reftable").exists() {
            return None;
        }
        let now = std::time::SystemTime::now();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut stamp = |path: &std::path::Path| -> Option<()> {
            path.hash(&mut hasher);
            match std::fs::metadata(path) {
                Ok(meta) => {
                    let modified = meta.modified().ok()?;
                    if modified + settle > now {
                        return None;
                    }
                    modified.hash(&mut hasher);
                    meta.len().hash(&mut hasher);
                    Some(())
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(()),
                Err(_) => None,
            }
        };
        stamp(&store.packed_refs_path())?;
        let mut folders = vec![common.join("refs")];
        if store.git_dir() != common {
            folders.push(store.git_dir().join("refs"));
        }
        while let Some(folder) = folders.pop() {
            stamp(&folder)?;
            let entries = match std::fs::read_dir(&folder) {
                Ok(entries) => entries,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return None,
            };
            let mut inner = Vec::new();
            for entry in entries {
                let entry = entry.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    inner.push(entry.path());
                }
            }
            inner.sort_unstable_by(|a, b| b.cmp(a));
            folders.extend(inner);
        }
        Some(hasher.finish())
    }

    /// Subject, author through `mailmap`, and the committer time's offset of `oid`.
    pub fn commit_text(&self, oid: &str, mailmap: &crate::Mailmap) -> Result<CommitText> {
        let id = gix::ObjectId::from_hex(oid.as_bytes())
            .map_err(|err| GitError::Internal(format!("not an object id {oid}: {err}")))?;
        self.commit_text_of(id, mailmap, &mut Names::default())
    }

    pub fn commit_text_of(
        &self,
        id: gix::ObjectId,
        mailmap: &crate::Mailmap,
        names: &mut Names,
    ) -> Result<CommitText> {
        Ok(self.read_text(id, mailmap, names)?.0)
    }

    pub(crate) fn read_text(
        &self,
        id: gix::ObjectId,
        mailmap: &crate::Mailmap,
        names: &mut Names,
    ) -> Result<(CommitText, i64)> {
        let commit = self
            .repo
            .find_commit(id)
            .map_err(|err| GitError::Internal(format!("cannot read commit: {err}")))?;

        let message = commit
            .message()
            .map_err(|err| GitError::Internal(format!("cannot read commit message: {err}")))?;
        let author = commit
            .author()
            .map_err(|err| GitError::Internal(format!("cannot read commit author: {err}")))?;
        let committer = commit
            .committer()
            .map_err(|err| GitError::Internal(format!("cannot read committer: {err}")))?;
        let time = committer
            .time()
            .map_err(|err| GitError::Internal(format!("cannot read commit time: {err}")))?;

        let (name, email) = (
            crate::search::lossy(author.name),
            crate::search::lossy(author.email),
        );
        let (author_name, author_email) = mailmap.canonical(&name, &email);
        let committer_identity = if author.name == committer.name && author.email == committer.email
        {
            None
        } else {
            let name = crate::search::lossy(committer.name);
            let email = crate::search::lossy(committer.email);
            distinct_committer(
                (author_name, author_email),
                mailmap.canonical(&name, &email),
            )
            .map(|identity| names.intern(&identity))
        };

        let text = CommitText {
            summary: message.summary().to_string(),
            author_name: names.intern(author_name),
            author_email: names.intern(author_email),
            committer_identity,
            tz_offset_minutes: time.offset / SECONDS_PER_MINUTE,
        };
        Ok((text, time.seconds))
    }
}

const REFS_SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

fn collect_tips<'a>(
    references: impl Iterator<Item = gix::Result<gix::Reference<'a>>>,
    tips: &mut Vec<gix::ObjectId>,
) {
    for reference in references.flatten() {
        if let Some(id) = reference.try_id() {
            tips.push(id.detach());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RepoHandle;
    use std::time::Duration;

    fn settled(f: &test_fixtures::Fixture) -> u64 {
        std::thread::sleep(Duration::from_millis(60));
        RepoHandle::open(f.path())
            .unwrap()
            .refs_stamp(Duration::ZERO)
            .unwrap()
    }

    #[test]
    fn the_stamp_holds_while_no_ref_moves() {
        let f = test_fixtures::linear(3).unwrap();
        f.git(&["update-ref", "refs/heads/topic/deep/x", "HEAD~1"])
            .unwrap();
        assert_eq!(settled(&f), settled(&f));
    }

    #[test]
    fn a_moved_loose_ref_changes_the_stamp() {
        let f = test_fixtures::linear(3).unwrap();
        f.git(&["update-ref", "refs/heads/topic/deep/x", "HEAD~1"])
            .unwrap();
        let mut seen = vec![settled(&f)];
        for args in [
            &["update-ref", "refs/heads/topic/deep/x", "HEAD~2"][..],
            &["update-ref", "refs/heads/main", "HEAD~1"],
            &["tag", "v1", "HEAD"],
            &["pack-refs", "--all"],
            &["update-ref", "refs/heads/topic/deep/x", "HEAD"],
            &["update-ref", "-d", "refs/tags/v1"],
        ] {
            f.git(args).unwrap();
            let stamp = settled(&f);
            assert!(!seen.contains(&stamp), "{args:?} left the stamp as it was");
            seen.push(stamp);
        }
    }

    #[test]
    fn a_change_younger_than_the_settle_time_is_not_vouched_for() {
        let f = test_fixtures::linear(1).unwrap();
        f.git(&["update-ref", "refs/heads/x", "HEAD"]).unwrap();
        let repo = RepoHandle::open(f.path()).unwrap();
        assert!(repo.refs_stamp(Duration::from_secs(3600)).is_none());
    }
}
