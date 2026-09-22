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

    /// One row from an id and its parents, whichever walk produced them.
    pub(crate) fn row_of(&self, id: gix::ObjectId, parents: &[gix::ObjectId]) -> Result<CommitRow> {
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

        Ok(CommitRow {
            oid: id.to_string(),
            parents: parents.iter().map(ToString::to_string).collect(),
            summary: message.summary().to_string(),
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            timestamp: time.seconds,
            tz_offset_minutes: time.offset / SECONDS_PER_MINUTE,
        })
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
