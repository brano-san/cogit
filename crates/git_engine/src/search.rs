use crate::topo::{LOOKAHEAD, in_date_order};
use crate::{CommitRow, GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRef {
    pub name: String,
    pub reason: String,
}

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
    /// Refs the References panel ticked; `None` is every ref, `Some([])` is none.
    #[serde(default)]
    pub visible_refs: Option<Vec<String>>,
}

impl CommitQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// A per-commit predicate forces a flat list; narrowing the ticked refs does not (R-51).
    #[must_use]
    pub fn filters_rows(&self) -> bool {
        Self {
            visible_refs: None,
            ..self.clone()
        } != Self::default()
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
            .map(drop)
    }

    /// Every tip peeled to a commit; a ref that names none is reported, not fatal (R-157).
    fn tips_for(&self, query: &CommitQuery) -> Result<(Vec<gix::ObjectId>, Vec<SkippedRef>)> {
        let Some(names) = query.visible_refs.as_deref() else {
            return Ok((self.graph_tips()?, Vec::new()));
        };

        let mut tips = Vec::with_capacity(names.len());
        let mut skipped = Vec::new();
        for rev in names {
            let Ok(id) = self.repo.rev_parse_single(rev.as_str()) else {
                tracing::debug!(rev, "a ticked ref no longer resolves");
                continue;
            };
            match id.object().map(gix::Object::peel_to_commit) {
                Ok(Ok(commit)) => tips.push(commit.id),
                _ => skipped.push(SkippedRef {
                    name: rev.clone(),
                    reason: "Tag does not point to a commit".to_owned(),
                }),
            }
        }
        if !skipped.is_empty() {
            tracing::warn!(skipped = skipped.len(), "ticked refs left out of the graph");
        }
        tips.sort_unstable();
        tips.dedup();
        Ok((tips, skipped))
    }

    /// Newest first by commit time, as `git log` reads: for search, history, Investigate.
    pub fn search_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let (tips, skipped) = self.tips_for(query)?;
        if !tips.is_empty() {
            self.stream_rows(
                query,
                chunk_size,
                in_date_order(self.by_date(tips)?, LOOKAHEAD),
                on_chunk,
            )?;
        }
        Ok(skipped)
    }

    /// Newest first by commit time; an unreadable object is logged and skipped.
    fn by_date(
        &self,
        tips: Vec<gix::ObjectId>,
    ) -> Result<impl Iterator<Item = (gix::ObjectId, Vec<gix::ObjectId>)> + '_> {
        Ok(self
            .repo
            .rev_walk(tips)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|err| GitError::Internal(format!("cannot walk history: {err}")))?
            .filter_map(|step| match step {
                Ok(info) => Some((info.id, info.parent_ids.into_iter().collect())),
                Err(err) => {
                    tracing::warn!(error = %err, "skipping an unreadable commit");
                    None
                }
            }))
    }

    fn stream_rows(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        walk: impl Iterator<Item = (gix::ObjectId, Vec<gix::ObjectId>)>,
        mut on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let chunk_size = chunk_size.max(1);
        let mut chunk = Vec::with_capacity(chunk_size);

        for (id, parents) in walk {
            let row = self.row_of(id, &parents)?;
            if !query.matches_row(&row) {
                continue;
            }
            // Last, because it costs two tree lookups per candidate.
            if let Some(path) = &query.path
                && !self.touches(&id, path)
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

    /// Whether the list for `query` holds this commit: a match streams by before its
    /// parents do, and the graph has to know then whether a line to them will end.
    pub fn shown_by(&self, query: &CommitQuery, oid: &str) -> bool {
        let Ok(id) = gix::ObjectId::from_hex(oid.as_bytes()) else {
            return false;
        };
        let Ok(commit) = self.repo.find_commit(id) else {
            return false;
        };
        let parents: Vec<gix::ObjectId> = commit.parent_ids().map(gix::Id::detach).collect();
        let Ok(row) = self.row_of(id, &parents) else {
            return false;
        };
        query.matches_row(&row)
            && query
                .path
                .as_deref()
                .is_none_or(|path| self.touches(&id, path))
    }

    /// Against the first parent, as `git log -- path` does before following renames.
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
