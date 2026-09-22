use crate::topo::{LOOKAHEAD, group_topologically};
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
    /// Revisions the References panel has ticked. `None` is every ref; `Some([])` is
    /// nothing, which is the honest answer when the user unticks the last box.
    #[serde(default)]
    pub visible_refs: Option<Vec<String>>,
}

impl CommitQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// True when a per-commit predicate is set, which forces a flat list. Narrowing the
    /// visible refs is not one: it drops whole tips, so every ancestor of a surviving tip
    /// is still there and the lanes between them stay truthful.
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
    }

    /// A tip that no longer resolves — a branch deleted while the panel still lists it —
    /// is dropped, not fatal. `rev_parse_single` rather than `find_reference`, because the
    /// panel also ticks stashes (`stash@{2}`) and lost commits (a bare oid).
    fn tips_for(&self, query: &CommitQuery) -> Result<Vec<gix::ObjectId>> {
        let Some(names) = query.visible_refs.as_deref() else {
            return self.graph_tips();
        };

        let mut tips: Vec<gix::ObjectId> = names
            .iter()
            .filter_map(|rev| match self.repo.rev_parse_single(rev.as_str()) {
                Ok(id) => Some(id.detach()),
                Err(err) => {
                    tracing::debug!(rev, error = %err, "a ticked ref no longer resolves");
                    None
                }
            })
            .collect();
        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
    }

    /// Newest first by commit time, the way `git log` reads by default. Search, history
    /// and Investigate all want this: a reader looking for "what happened on Tuesday"
    /// wants Tuesday, not one line of history at a time.
    pub fn search_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let tips = self.tips_for(query)?;
        if tips.is_empty() {
            return Ok(());
        }

        self.stream_rows(query, chunk_size, self.by_date(tips)?, on_chunk)
    }

    /// The same commits, ordered so that no line of history is interleaved with another —
    /// `git log --topo-order`, computed in a sliding window over the date walk. The graph
    /// is drawn from this: lanes only stay straight if a branch's commits arrive together
    /// (doc/12-risks.md, R-140).
    pub fn search_commits_topo(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let tips = self.tips_for(query)?;
        if tips.is_empty() {
            return Ok(());
        }

        let walk = group_topologically(self.by_date(tips)?, LOOKAHEAD);
        self.stream_rows(query, chunk_size, walk, on_chunk)
    }

    /// Newest first by commit time. An unreadable object is skipped with a line in the
    /// log: one bad commit must not end the history.
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

    /// Filtering and chunking, shared by both walks.
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
