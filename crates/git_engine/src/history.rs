use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub oid: String,
    /// Git order: the first parent is the mainline, which lane allocation relies on.
    pub parents: Vec<String>,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    /// specta refuses `i64`; Unix seconds stay far below 2^53, so a number is exact.
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub tz_offset_minutes: i32,
}

/// The part of a row read from the commit object itself; the graph reads it for the rows
/// on screen only (R-302).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitText {
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub tz_offset_minutes: i32,
}

const SECONDS_PER_MINUTE: i32 = 60;

impl RepoHandle {
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

        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
    }

    /// Every ref and where HEAD points, hashed, with what each reflog selector among
    /// `visible_refs` names: `stash drop stash@{1}` moves no ref, yet `stash@{1}` is another
    /// commit after it. The shallow boundary too: `fetch --unshallow` moves no ref either.
    /// Equal prints mean the graph walk would start from the same tips and stop at the same
    /// commits; only the ref store, the reflogs and `shallow` are read, never a commit.
    pub fn refs_fingerprint(&self, visible_refs: Option<&[String]>) -> Result<u64> {
        use std::hash::{Hash as _, Hasher as _};
        fn add(hasher: &mut impl std::hash::Hasher, reference: &gix::Reference<'_>) {
            reference.name().as_bstr().hash(hasher);
            match reference.target() {
                gix::refs::TargetRef::Object(id) => id.as_bytes().hash(hasher),
                gix::refs::TargetRef::Symbolic(name) => name.as_bstr().hash(hasher),
            }
        }

        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for reference in platform
            .all()
            .map_err(|err| GitError::Internal(format!("cannot list references: {err}")))?
        {
            let reference = reference
                .map_err(|err| GitError::Internal(format!("cannot read a reference: {err}")))?;
            add(&mut hasher, &reference);
        }
        if let Ok(head) = self.repo.find_reference("HEAD") {
            add(&mut hasher, &head);
        }
        for rev in visible_refs.into_iter().flatten() {
            if rev.contains("@{") {
                rev.hash(&mut hasher);
                if let Ok(id) = self.repo.rev_parse_single(rev.as_str()) {
                    id.as_bytes().hash(&mut hasher);
                }
            }
        }
        self.shallow_commits().hash(&mut hasher);
        Ok(hasher.finish())
    }

    /// One row from an id and its parents, whichever walk produced them.
    pub(crate) fn row_of(
        &self,
        id: gix::ObjectId,
        parents: &[gix::ObjectId],
        mailmap: &crate::Mailmap,
    ) -> Result<CommitRow> {
        let (text, timestamp) = self.read_text(id, mailmap)?;
        Ok(CommitRow {
            oid: id.to_string(),
            parents: parents.iter().map(ToString::to_string).collect(),
            summary: text.summary,
            author_name: text.author_name,
            author_email: text.author_email,
            timestamp,
            tz_offset_minutes: text.tz_offset_minutes,
        })
    }

    /// Subject, author through `mailmap`, and the committer time's offset of `oid`.
    pub fn commit_text(&self, oid: &str, mailmap: &crate::Mailmap) -> Result<CommitText> {
        let id = gix::ObjectId::from_hex(oid.as_bytes())
            .map_err(|err| GitError::Internal(format!("not an object id {oid}: {err}")))?;
        Ok(self.read_text(id, mailmap)?.0)
    }

    fn read_text(&self, id: gix::ObjectId, mailmap: &crate::Mailmap) -> Result<(CommitText, i64)> {
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
        let time = commit
            .time()
            .map_err(|err| GitError::Internal(format!("cannot read commit time: {err}")))?;

        let (mut author_name, mut author_email) =
            (author.name.to_string(), author.email.to_string());
        mailmap.apply(&mut author_name, &mut author_email);

        let text = CommitText {
            summary: message.summary().to_string(),
            author_name,
            author_email,
            tz_offset_minutes: time.offset / SECONDS_PER_MINUTE,
        };
        Ok((text, time.seconds))
    }
}

fn collect_tips<'a>(
    references: impl Iterator<
        Item = std::result::Result<gix::Reference<'a>, Box<dyn std::error::Error + Send + Sync>>,
    >,
    tips: &mut Vec<gix::ObjectId>,
) {
    for reference in references.flatten() {
        if let Some(id) = reference.try_id() {
            tips.push(id.detach());
        }
    }
}
