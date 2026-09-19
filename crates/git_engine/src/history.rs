use crate::{GitError, RepoHandle, Result};
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
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
    /// Seconds since the Unix epoch.
    ///
    /// specta refuses `i64` by default to guard against precision loss. Unix seconds
    /// stay far below 2^53, so a plain TypeScript number is exact here.
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub tz_offset_minutes: i32,
}

const SECONDS_PER_MINUTE: i32 = 60;

impl RepoHandle {
    pub fn stream_commits(
        &self,
        chunk_size: usize,
        mut on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let chunk_size = chunk_size.max(1);
        let tips = self.graph_tips()?;
        if tips.is_empty() {
            return Ok(());
        }

        let walk = self
            .repo
            .rev_walk(tips)
            .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
            .all()
            .map_err(|err| GitError::Internal(format!("cannot walk history: {err}")))?;

        let mut chunk = Vec::with_capacity(chunk_size);
        for info in walk {
            let info = match info {
                Ok(info) => info,
                Err(err) => {
                    tracing::warn!(error = %err, "skipping an unreadable commit");
                    continue;
                }
            };

            chunk.push(self.to_row(&info)?);
            if chunk.len() >= chunk_size {
                let full = std::mem::replace(&mut chunk, Vec::with_capacity(chunk_size));
                if !on_chunk(full) {
                    return Ok(());
                }
            }
        }

        if !chunk.is_empty() {
            on_chunk(chunk);
        }
        Ok(())
    }

    fn graph_tips(&self) -> Result<Vec<gix::ObjectId>> {
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

    fn to_row(&self, info: &gix::revision::walk::Info<'_>) -> Result<CommitRow> {
        let commit = info
            .object()
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
            oid: info.id.to_string(),
            parents: info.parent_ids.iter().map(ToString::to_string).collect(),
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
