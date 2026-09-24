use crate::graph_walk::{ByTime, CommitReader, Reuse, WalkedHistory};
use crate::topo::{LOOKAHEAD, in_date_order};
use crate::{CommitRow, RepoHandle, Result};
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
    /// Every tip peeled to a commit; a ref that names none is reported, not fatal (R-157).
    pub(crate) fn tips_for(
        &self,
        query: &CommitQuery,
    ) -> Result<(Vec<gix::ObjectId>, Vec<SkippedRef>)> {
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
        self.graph_commits(query, chunk_size, None, None, on_chunk)
    }

    /// `search_commits` for the graph: a commit `reuse` holds is copied from there, not
    /// read, and `record` lists every row this walk gives for the walk after it (R-301).
    pub fn graph_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        reuse: Option<Reuse<'_>>,
        record: Option<&mut WalkedHistory>,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let (tips, skipped) = self.tips_for(query)?;
        if !tips.is_empty() {
            let head = self.first_of(&tips);
            let reader = CommitReader::new(&self.repo);
            let read = |id| {
                reuse
                    .and_then(|reuse| reuse.read(&id))
                    .or_else(|| reader.read(id))
            };
            let walk = ByTime::new(tips, read, false, self.shallow_commits());
            let rows = Rows { reuse, record };
            self.stream_rows(
                query,
                chunk_size,
                in_date_order(walk, LOOKAHEAD, head),
                rows,
                on_chunk,
            )?;
        }
        Ok(skipped)
    }

    /// HEAD, when it is a tip: `first` has to be in the walk, or all of it is read up front.
    pub(crate) fn first_of(&self, tips: &[gix::ObjectId]) -> Option<gix::ObjectId> {
        self.repo
            .head_id()
            .ok()
            .map(gix::Id::detach)
            .filter(|head| tips.contains(head))
    }

    fn stream_rows(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        walk: impl Iterator<Item = (gix::ObjectId, Vec<gix::ObjectId>)>,
        mut rows: Rows<'_, '_>,
        mut on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let chunk_size = chunk_size.max(1);
        let mut chunk = Vec::with_capacity(chunk_size);
        let mut listed = 0_u32;

        for (id, parents) in walk {
            let row = match rows.reuse.and_then(|reuse| reuse.row(&id)) {
                Some(row) => row.clone(),
                None => self.row_of(id, &parents)?,
            };
            if !query.matches_row(&row) {
                continue;
            }
            // Last, because it costs two tree lookups per candidate.
            if let Some(path) = &query.path
                && !self.touches(&id, path)
            {
                continue;
            }

            if let Some(record) = rows.record.as_deref_mut() {
                record.push(id, listed, row.timestamp, parents);
            }
            listed = listed.saturating_add(1);
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

/// Where a walk's rows come from besides the objects, and where it lists them.
struct Rows<'r, 'h> {
    reuse: Option<Reuse<'r>>,
    record: Option<&'h mut WalkedHistory>,
}
