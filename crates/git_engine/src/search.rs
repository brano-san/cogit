use crate::graph_walk::{ByTime, CommitReader, CutParents, Reuse, WalkedHistory};
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
    /// How the graph shows the walked history; a filtered list ignores it.
    #[serde(default)]
    pub view: GraphView,
    /// Not a filter: how the graph this load lays out cuts long links (R-330).
    #[serde(default)]
    pub long_link_rows: Option<u32>,
}

/// Graph modes that decide which commits the graph shows (`graph_engine::ViewFilter`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphView {
    /// `--first-parent`: one line per ticked ref, merged branches left out.
    pub first_parent: bool,
    /// A merged branch is one row at its merge, but for the merges in `expanded`.
    pub collapse_merged: bool,
    pub expanded: Vec<String>,
}

impl CommitQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        Self {
            long_link_rows: None,
            ..self.clone()
        } == Self::default()
    }

    /// A per-commit predicate forces a flat list; narrowing the ticked refs or following
    /// first parents only does not (R-51).
    #[must_use]
    pub fn filters_rows(&self) -> bool {
        Self {
            visible_refs: None,
            view: GraphView::default(),
            long_link_rows: None,
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
    /// The commits the walk for `query` starts from, as hex ids.
    pub fn walk_tips(&self, query: &CommitQuery) -> Result<Vec<String>> {
        Ok(self
            .tips_for(query)?
            .ids
            .iter()
            .map(ToString::to_string)
            .collect())
    }

    pub(crate) fn tips_for(&self, query: &CommitQuery) -> Result<Tips> {
        let Some(names) = query.visible_refs.as_deref() else {
            return Ok(Tips {
                ids: self.graph_tips()?,
                ..Tips::default()
            });
        };

        let mut tips = Tips::default();
        for rev in names {
            let Ok(id) = self.repo.rev_parse_single(rev.as_str()) else {
                tracing::debug!(rev, "a ticked ref no longer resolves");
                continue;
            };
            match id.object().map(gix::Object::peel_to_commit) {
                Ok(Ok(commit)) => {
                    if is_stash(rev) {
                        tips.stashes.push(commit.id);
                    }
                    tips.ids.push(commit.id);
                }
                _ => tips.skipped.push(SkippedRef {
                    name: rev.clone(),
                    reason: "Tag does not point to a commit".to_owned(),
                }),
            }
        }
        if !tips.skipped.is_empty() {
            tracing::warn!(
                skipped = tips.skipped.len(),
                "ticked refs left out of the graph"
            );
        }
        tips.ids.sort_unstable();
        tips.ids.dedup();
        tips.stashes.sort_unstable();
        tips.stashes.dedup();
        Ok(tips)
    }

    /// Newest first by commit time, as `git log` reads: for search, history, Investigate.
    pub fn search_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let rows = GraphRows {
            reuse: None,
            record: None,
            text: true,
            cut: None,
        };
        self.graph_commits(query, chunk_size, rows, on_chunk)
    }

    /// `search_commits` for the graph: time and parents of a commit `reuse` holds come from
    /// there (R-301); `record` lists every row for the walk after it; without `text`, rows
    /// carry no subject or author, only what the walk read (R-302); `cut` hears of the
    /// parents the walk will never list, before the row that names them.
    pub fn graph_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        rows: GraphRows<'_, '_>,
        on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let GraphRows {
            reuse,
            record,
            text,
            cut,
        } = rows;
        // Rows without text cannot be matched against a filter.
        let text = text || query.filters_rows();
        let Tips {
            ids: tips,
            skipped,
            stashes,
        } = self.tips_for(query)?;
        if !tips.is_empty() {
            let head = self.first_of(&tips);
            let reader = CommitReader::new(&self.repo);
            let read = |id| {
                let found = reuse
                    .and_then(|reuse| reuse.read(&id))
                    .or_else(|| reader.read(id));
                if found.is_none()
                    && let Some(cut) = cut
                {
                    cut.insert(id);
                }
                found
            };
            // A filtered list shows matches from every line, the merged ones too (#26).
            let first_parent = query.view.first_parent && !query.filters_rows();
            let shallow = self.shallow_commits();
            let walk = ByTime::new(tips, read, first_parent, shallow.clone())
                .first_parent_of(stashes.clone())
                .inspect(|(id, parents, _)| {
                    if let Some(cut) = cut
                        && shallow.binary_search(id).is_ok()
                    {
                        parents.iter().for_each(|parent| cut.insert(*parent));
                    }
                });
            let rows = Rows {
                record,
                text,
                stashes: &stashes,
            };
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
        walk: impl Iterator<Item = (gix::ObjectId, Vec<gix::ObjectId>, i64)>,
        mut rows: Rows<'_>,
        mut on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<()> {
        let chunk_size = chunk_size.max(1);
        let mut chunk = Vec::with_capacity(chunk_size);
        // Once per walk: a `stat` per row would cost more than reading the commit.
        let mailmap = if rows.text {
            self.mailmap()
        } else {
            std::sync::Arc::default()
        };

        for (id, parents, time) in walk {
            let row = if rows.text {
                self.row_of(id, &parents, &mailmap)?
            } else {
                CommitRow {
                    oid: id.to_string(),
                    parents: parents.iter().map(ToString::to_string).collect(),
                    summary: String::new(),
                    author_name: String::new(),
                    author_email: String::new(),
                    timestamp: time,
                    tz_offset_minutes: 0,
                }
            };
            let mut row = row;
            // The walk follows a stash's first parent only; its other lines would go nowhere.
            if !rows.stashes.is_empty() && rows.stashes.binary_search(&id).is_ok() {
                row.parents.truncate(1);
            }
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
                record.push(id, row.timestamp, parents);
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
        self.shown_by_with(query, oid, &self.mailmap())
    }

    /// `shown_by` for a loop over many commits, with the mailmap read once for all of them.
    pub fn shown_by_with(&self, query: &CommitQuery, oid: &str, mailmap: &crate::Mailmap) -> bool {
        let Ok(id) = gix::ObjectId::from_hex(oid.as_bytes()) else {
            return false;
        };
        let Ok(commit) = self.repo.find_commit(id) else {
            return false;
        };
        let parents: Vec<gix::ObjectId> = commit.parent_ids().map(gix::Id::detach).collect();
        let Ok(row) = self.row_of(id, &parents, mailmap) else {
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

/// Where a walk lists its rows, and whether it reads their text.
struct Rows<'h> {
    record: Option<&'h mut WalkedHistory>,
    text: bool,
    /// Sorted: ticked stashes, one row each with only the first parent (F-331).
    stashes: &'h [gix::ObjectId],
}

/// The commits a walk starts from.
#[derive(Debug, Default)]
pub(crate) struct Tips {
    pub(crate) ids: Vec<gix::ObjectId>,
    pub(crate) skipped: Vec<SkippedRef>,
    /// Sorted. Ticked by a `refs/stash` selector: `git stash` keeps the index and the
    /// untracked files in commits of its own, second and third parents that are no history.
    pub(crate) stashes: Vec<gix::ObjectId>,
}

/// `stash@{N}`, `refs/stash@{N}` or `refs/stash` itself.
fn is_stash(rev: &str) -> bool {
    rev == "refs/stash" || rev.starts_with("stash@{") || rev.starts_with("refs/stash@{")
}

/// How a graph walk gets and gives its rows (`graph_commits`).
#[derive(Debug)]
pub struct GraphRows<'r, 'h> {
    pub reuse: Option<Reuse<'r>>,
    pub record: Option<&'h mut WalkedHistory>,
    pub text: bool,
    pub cut: Option<&'h CutParents>,
}
