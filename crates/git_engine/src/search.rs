use crate::{CommitRow, GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct CommitQuery {
    pub author: Option<String>,
    pub message: Option<String>,
    pub oid_prefix: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub since: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub until: Option<i64>,
    pub path: Option<String>,
}

impl CommitQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn matches_row(&self, row: &CommitRow) -> bool {
        if let Some(message) = &self.message
            && !contains_ignoring_case(&row.summary, message)
        {
            return false;
        }
        if let Some(author) = &self.author
            && !contains_ignoring_case(&row.author_name, author)
            && !contains_ignoring_case(&row.author_email, author)
        {
            return false;
        }
        if let Some(prefix) = &self.oid_prefix
            && !row.oid.starts_with(&prefix.to_ascii_lowercase())
        {
            return false;
        }
        if self.since.is_some_and(|since| row.timestamp < since) {
            return false;
        }
        if self.until.is_some_and(|until| row.timestamp > until) {
            return false;
        }
        true
    }
}

impl RepoHandle {
    pub fn stream_commits(
        &self,
        chunk_size: usize,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        self.search_commits(&CommitQuery::default(), chunk_size, on_chunk)
    }

    pub fn search_commits(
        &self,
        query: &CommitQuery,
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
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
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

            let row = self.to_row(&info)?;
            if !query.matches_row(&row) {
                continue;
            }
            // Last, because it costs two tree lookups per candidate.
            if let Some(path) = &query.path
                && !self.touches(&info.id, path)
            {
                continue;
            }

            chunk.push(row);
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

    /// Compares the entry at `path` with the same entry in the first parent, which is what
    /// `git log -- path` does before rename following.
    fn touches(&self, oid: &gix::ObjectId, path: &str) -> bool {
        let Ok(commit) = self.repo.find_commit(*oid) else {
            return false;
        };
        let current = self.entry_id(&commit, path);

        match commit.parent_ids().next() {
            None => current.is_some(),
            Some(parent) => {
                let before = self
                    .repo
                    .find_commit(parent.detach())
                    .ok()
                    .and_then(|parent| self.entry_id(&parent, path));
                current != before
            }
        }
    }

    fn entry_id(&self, commit: &gix::Commit<'_>, path: &str) -> Option<gix::ObjectId> {
        commit
            .tree()
            .ok()?
            .lookup_entry_by_path(path)
            .ok()
            .flatten()
            .map(|entry| entry.id().detach())
    }
}

fn contains_ignoring_case(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}
