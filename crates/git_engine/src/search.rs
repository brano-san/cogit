use rayon::prelude::*;

use crate::graph_walk::{ByTime, CommitReader, CutParents, Reuse, WalkedHistory};
use crate::text_search::{TextMatch, contains_lowered};
use crate::topo::{LOOKAHEAD, in_date_order, in_graph_order, merged_after_head};
use crate::{CommitRow, CommitText, Mailmap, RepoHandle, Result, TextFields};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRef {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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
    /// The filter's free text, looked for in the fields of `text_in` (F-560).
    pub text: Option<String>,
    #[serde(default)]
    pub text_in: TextFields,
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

/// Graph modes that decide which commits the graph shows: the walk follows first parents,
/// `graph_engine::ViewFilter` collapses merged branches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphView {
    /// `--first-parent`: one line per ticked ref, merged branches left out.
    pub first_parent: bool,
    /// A merged branch is one row at its merge, but for the merges in `expanded`.
    pub collapse_merged: bool,
    pub expanded: Vec<String>,
    /// Show Graph While Filtering: a filtered list keeps lines between its matches (R-575).
    pub filtered_graph: bool,
    /// `--date-order` instead of the default `--topo-order` (R-766).
    pub date_order: bool,
}

/// Commits a name or content search tests at once, and the share one thread takes (R-625).
const SCAN_BATCH: usize = 1024;
const SCAN_CHUNK: usize = 16;

impl CommitQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        Self {
            long_link_rows: None,
            text_in: TextFields::default(),
            ..self.clone()
        } == Self::default()
    }

    /// A per-commit predicate forces a flat list; narrowing the ticked refs or following
    /// first parents only does not (R-51). The fields to search mean nothing without text.
    #[must_use]
    pub fn filters_rows(&self) -> bool {
        Self {
            visible_refs: None,
            view: GraphView::default(),
            long_link_rows: None,
            text_in: TextFields::default(),
            ..self.clone()
        } != Self::default()
    }
}

/// A query's per-commit fields, lowered once per walk instead of once per row.
#[derive(Debug)]
struct RowFilter {
    message: Option<String>,
    author: Option<String>,
    oid_prefix: Option<String>,
    since: Option<i64>,
    until: Option<i64>,
}

impl RowFilter {
    fn of(query: &CommitQuery) -> Self {
        Self {
            message: query.message.as_deref().map(str::to_lowercase),
            author: query.author.as_deref().map(str::to_lowercase),
            oid_prefix: query.oid_prefix.as_deref().map(str::to_ascii_lowercase),
            since: query.since,
            until: query.until,
        }
    }

    fn matches(&self, commit: &GraphCommit) -> bool {
        let text = &commit.text;
        if let Some(message) = &self.message
            && !contains_lowered(&text.summary, message)
        {
            return false;
        }
        if let Some(author) = &self.author
            && !contains_lowered(&text.author_name, author)
            && !contains_lowered(&text.author_email, author)
        {
            return false;
        }
        if let Some(prefix) = &self.oid_prefix {
            let mut hex = [0_u8; 64];
            if !commit.id.hex_to_buf(&mut hex).starts_with(prefix.as_str()) {
                return false;
            }
        }
        if self.since.is_some_and(|since| commit.time < since) {
            return false;
        }
        if self.until.is_some_and(|until| commit.time > until) {
            return false;
        }
        true
    }
}

/// A walked commit as the graph takes it: ids, not hex; `text` stays empty unless read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCommit {
    pub id: gix::ObjectId,
    /// Git order: the first parent is the mainline, which lane allocation relies on.
    pub parents: crate::Parents,
    pub time: i64,
    pub text: CommitText,
}

impl GraphCommit {
    /// The row search and history hand out: hex at the IPC edge.
    #[must_use]
    pub fn into_row(self) -> CommitRow {
        CommitRow {
            oid: self.id.to_string(),
            parents: self.parents.iter().map(ToString::to_string).collect(),
            summary: self.text.summary,
            author_name: self.text.author_name,
            author_email: self.text.author_email,
            timestamp: self.time,
            tz_offset_minutes: self.text.tz_offset_minutes,
        }
    }
}

impl RepoHandle {
    /// Every tip peeled to a commit; a ref that names none is reported, not fatal (R-157).
    /// The commits the walk for `query` starts from, as hex ids.
    pub fn walk_tips(&self, query: &CommitQuery) -> Result<Vec<gix::ObjectId>> {
        Ok(self.tips_for(query)?.ids)
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
                    tips.ids.push(commit.id);
                }
                _ => tips.skipped.push(SkippedRef {
                    name: rev.clone(),
                    reason: "Tag does not point to a commit".to_owned(),
                }),
            }
        }
        // With HEAD comes the branch a merge in progress takes in: drawn as merged (07 §4).
        if names.iter().any(|name| name == "HEAD") {
            tips.ids.extend(self.merge_head());
        }
        if !tips.skipped.is_empty() {
            tracing::warn!(
                skipped = tips.skipped.len(),
                "ticked refs left out of the graph"
            );
        }
        tips.ids.sort_unstable();
        tips.ids.dedup();
        Ok(tips)
    }

    /// In the graph's order (`query.view`): for search, history, Investigate.
    pub fn search_commits(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        mut on_chunk: impl FnMut(Vec<CommitRow>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let rows = GraphRows {
            reuse: None,
            record: None,
            text: true,
            cut: None,
        };
        self.graph_commits(query, chunk_size, rows, |chunk| {
            on_chunk(chunk.into_iter().map(GraphCommit::into_row).collect())
        })
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
        on_chunk: impl FnMut(Vec<GraphCommit>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        self.graph_commits_passing(query, chunk_size, rows, None, on_chunk)
    }

    /// `graph_commits` that lists in `passed` the commits the filter left out, in order.
    pub fn graph_commits_passing(
        &self,
        query: &CommitQuery,
        chunk_size: usize,
        rows: GraphRows<'_, '_>,
        passed: Option<&PassedCommits>,
        on_chunk: impl FnMut(Vec<GraphCommit>) -> bool,
    ) -> Result<Vec<SkippedRef>> {
        let GraphRows {
            reuse,
            record,
            text,
            cut,
        } = rows;
        // Rows without text cannot be matched against a filter.
        let text = text || query.filters_rows();
        let Tips { ids: tips, skipped } = self.tips_for(query)?;
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
            let walk =
                ByTime::new(tips, read, first_parent, shallow.clone()).inspect(|(id, _, _)| {
                    if let Some(cut) = cut
                        && shallow.binary_search(id).is_ok()
                    {
                        cut.insert_shallow(*id);
                    }
                });
            let rows = Rows {
                record,
                text,
                passed,
            };
            // A filtered list keeps the graph's order: it only leaves rows out.
            let merging = head.zip(self.merge_head()).filter(|_| !first_parent);
            let ordered: Box<dyn Iterator<Item = _>> = match (merging, query.view.date_order) {
                (Some((head, merge)), true) => Box::new(
                    in_date_order(merged_after_head(walk, head, merge), LOOKAHEAD, Some(head))
                        .map(|ordered| ordered.item),
                ),
                (Some((head, merge)), false) => Box::new(
                    in_graph_order(
                        merged_after_head(walk, head, merge),
                        Some(head),
                        first_parent,
                    )
                    .map(|ordered| ordered.item),
                ),
                (None, true) => Box::new(in_date_order(walk, LOOKAHEAD, head)),
                (None, false) => Box::new(in_graph_order(walk, head, first_parent)),
            };
            self.stream_rows(query, chunk_size, ordered, rows, on_chunk)?;
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
        walk: impl Iterator<Item = (gix::ObjectId, crate::Parents, i64)>,
        mut rows: Rows<'_>,
        mut on_chunk: impl FnMut(Vec<GraphCommit>) -> bool,
    ) -> Result<()> {
        let chunk_size = chunk_size.max(1);
        let mut chunk = Vec::with_capacity(chunk_size);
        // Once per walk: a `stat` per row would cost more than reading the commit.
        let mailmap = if rows.text {
            self.mailmap()
        } else {
            std::sync::Arc::default()
        };
        let filter = RowFilter::of(query);
        let text = self.text_match(query);
        let heavy = text.as_ref().is_some_and(TextMatch::is_heavy);
        let shared = heavy.then(|| self.repo.clone().into_sync());
        let mut walk = walk;
        // Kept across batches: a batch is one commit unless the filter is heavy.
        let mut read: Vec<GraphCommit> = Vec::new();
        let mut hits: Vec<bool> = Vec::new();

        loop {
            // Diffing trees costs far more than walking, so a batch of those is tested at
            // once on every core; the rows still leave in walk order.
            let mut failure = None;
            for (id, parents, time) in walk.by_ref().take(if heavy { SCAN_BATCH } else { 1 }) {
                match self.graph_commit(id, parents, time, rows.text, &mailmap) {
                    Ok(commit) => read.push(commit),
                    Err(err) => {
                        failure = Some(err);
                        break;
                    }
                }
            }
            if read.is_empty() && failure.is_none() {
                break;
            }
            let passes = |repo: &gix::Repository, commit: &GraphCommit| {
                filter.matches(commit)
                    && text
                        .as_ref()
                        .is_none_or(|text| text.matches(repo, commit, &mailmap))
            };
            hits.clear();
            match &shared {
                Some(shared) if read.len() > 1 => hits.extend(
                    read.par_chunks(SCAN_CHUNK)
                        .map(|part| {
                            let repo = shared.to_thread_local();
                            part.iter()
                                .map(|commit| passes(&repo, commit))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                        .concat(),
                ),
                _ => hits.extend(read.iter().map(|commit| passes(&self.repo, commit))),
            }

            for (commit, &hit) in read.drain(..).zip(&hits) {
                // The path last, because it costs two tree lookups per candidate.
                let shown = hit
                    && query
                        .path
                        .as_ref()
                        .is_none_or(|path| self.touches(&commit.id, path));
                if !shown {
                    if let Some(passed) = rows.passed {
                        passed.0.borrow_mut().push(Passed {
                            before: chunk.len(),
                            oid: commit.id,
                            first_parent: commit.parents.first().copied(),
                        });
                    }
                    continue;
                }

                if let Some(record) = rows.record.as_deref_mut() {
                    record.push(commit.id, commit.time, &commit.parents);
                }
                chunk.push(commit);
                if chunk.len() >= chunk_size {
                    let full = std::mem::replace(&mut chunk, Vec::with_capacity(chunk_size));
                    if !on_chunk(full) {
                        return Ok(());
                    }
                }
            }
            if let Some(err) = failure {
                return Err(err);
            }
        }

        if !chunk.is_empty() {
            on_chunk(chunk);
        }
        Ok(())
    }

    /// The walk's commit with its text when `text`; the time is the object's then, as the
    /// row of a search has always had it.
    fn graph_commit(
        &self,
        id: gix::ObjectId,
        parents: crate::Parents,
        time: i64,
        text: bool,
        mailmap: &Mailmap,
    ) -> Result<GraphCommit> {
        if !text {
            return Ok(GraphCommit {
                id,
                parents,
                time,
                text: CommitText::default(),
            });
        }
        let (text, time) = self.read_text(id, mailmap)?;
        Ok(GraphCommit {
            id,
            parents,
            time,
            text,
        })
    }

    /// Whether the list for `query` holds this commit: a match streams by before its
    /// parents do, and the graph has to know then whether a line to them will end.
    pub fn shown_by(&self, query: &CommitQuery, oid: &str) -> bool {
        self.shown_filter(query).shows(self, oid)
    }

    /// `shown_by` for many commits: the mailmap and the ref names are read once for all.
    #[must_use]
    pub fn shown_filter<'q>(&self, query: &'q CommitQuery) -> ShownBy<'q> {
        ShownBy {
            query,
            filter: RowFilter::of(query),
            mailmap: self.mailmap(),
            text: self.text_match(query),
        }
    }

    /// Against the first parent, as `git log -- path` does before following renames.
    fn touches(&self, oid: &gix::ObjectId, path: &str) -> bool {
        let Ok(commit) = self.repo.find_commit(*oid) else {
            return false;
        };
        let current = self.entry_of(&commit, path);

        match commit.parent_ids().next() {
            None => current.is_some(),
            Some(parent) => {
                let before = self
                    .repo
                    .find_commit(parent.detach())
                    .ok()
                    .and_then(|parent| self.entry_of(&parent, path));
                current != before
            }
        }
    }

    /// Mode and id: `git log -- path` counts a commit that only makes a file executable.
    fn entry_of(
        &self,
        commit: &gix::Commit<'_>,
        path: &str,
    ) -> Option<(gix::object::tree::EntryMode, gix::ObjectId)> {
        commit
            .tree()
            .ok()?
            .lookup_entry_by_path(path)
            .ok()
            .flatten()
            .map(|entry| (entry.mode(), entry.id().detach()))
    }
}

/// Whether commits are in the list for one query (`RepoHandle::shown_filter`).
#[derive(Debug)]
pub struct ShownBy<'q> {
    query: &'q CommitQuery,
    filter: RowFilter,
    mailmap: std::sync::Arc<Mailmap>,
    text: Option<TextMatch>,
}

impl ShownBy<'_> {
    #[must_use]
    pub fn shows(&self, handle: &RepoHandle, oid: &str) -> bool {
        gix::ObjectId::from_hex(oid.as_bytes()).is_ok_and(|id| self.shows_id(handle, id))
    }

    #[must_use]
    pub fn shows_id(&self, handle: &RepoHandle, id: gix::ObjectId) -> bool {
        let Ok((text, time)) = handle.read_text(id, &self.mailmap) else {
            return false;
        };
        let commit = GraphCommit {
            id,
            parents: crate::Parents::new(),
            time,
            text,
        };
        self.filter.matches(&commit)
            && self
                .text
                .as_ref()
                .is_none_or(|text| text.matches(&handle.repo, &commit, &self.mailmap))
            && self
                .query
                .path
                .as_deref()
                .is_none_or(|path| handle.touches(&id, path))
    }
}

/// Where a walk lists its rows, and whether it reads their text.
struct Rows<'h> {
    record: Option<&'h mut WalkedHistory>,
    text: bool,
    passed: Option<&'h PassedCommits>,
}

/// A commit a filter left out, before the row `before` of the chunk being filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passed {
    pub before: usize,
    pub oid: gix::ObjectId,
    pub first_parent: Option<gix::ObjectId>,
}

/// What a filtered walk left out, taken chunk by chunk for lines drawn through it (R-575).
#[derive(Debug, Default)]
pub struct PassedCommits(std::cell::RefCell<Vec<Passed>>);

impl PassedCommits {
    #[must_use]
    pub fn take(&self) -> Vec<Passed> {
        std::mem::take(&mut self.0.borrow_mut())
    }
}

/// The commits a walk starts from.
#[derive(Debug, Default)]
pub(crate) struct Tips {
    pub(crate) ids: Vec<gix::ObjectId>,
    pub(crate) skipped: Vec<SkippedRef>,
}

/// How a graph walk gets and gives its rows (`graph_commits`).
#[derive(Debug)]
pub struct GraphRows<'r, 'h> {
    pub reuse: Option<Reuse<'r>>,
    pub record: Option<&'h mut WalkedHistory>,
    pub text: bool,
    pub cut: Option<&'h CutParents>,
}
