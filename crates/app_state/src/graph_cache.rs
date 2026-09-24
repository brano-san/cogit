//! The laid-out graph stays on this side; the UI asks for the rows it shows (R-193). One
//! is kept per repository shown recently, so switching back is a lookup, not a walk (R-300);
//! a new walk takes time and parents from the last one instead of reading them (R-301); and
//! subject and author are read only for the rows a window hands out (R-302).

use crate::graph_overlay::PaintMemo;
use crate::{AppState, GraphChunk, RepoId};
use git_engine::{
    CommitQuery, CommitRow, CommitText, GitError, GraphRows, Mailmap, RepoHandle, Reuse,
    SkippedRef, WalkedHistory,
};
use graph_engine::GraphRow;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Graphs stay until together they pass this; the one asked for last always stays. A
/// 50 000-commit history takes about 20 MB: four such repositories, or dozens of usual ones.
const GRAPH_CACHE_BYTES: usize = 96 << 20;

/// How far the walk got. The rows themselves travel only when asked for, by window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphProgress {
    pub generation: u32,
    /// Rows laid out so far: the list is this long while the rest is being walked.
    pub total: u32,
    pub is_last: bool,
    /// The graph this one replaces; its first `kept` rows are these rows, row for row, so
    /// the blocks already fetched of them stay good (R-301).
    pub base: Option<u32>,
    pub kept: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphWindow {
    pub start: u32,
    pub total: u32,
    pub complete: bool,
    pub commits: Vec<CommitRow>,
    /// One per commit, in the same order.
    pub rows: Vec<GraphRow>,
}

#[derive(Debug, Default, Clone)]
struct Laid {
    commits: Vec<CommitRow>,
    rows: Vec<GraphRow>,
    /// Filled once the walk is over; what the next walk takes time and parents from.
    history: WalkedHistory,
    /// Merges folded by `collapse_merged` and how many commits each holds (#26).
    folds: BTreeMap<u32, u32>,
}

impl Laid {
    fn total(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
    }
}

/// Subject and author of the rows handed out so far, through one mailmap (R-390).
#[derive(Debug)]
struct Texts {
    mailmap: Arc<Mailmap>,
    rows: HashMap<usize, CommitText>,
}

impl Texts {
    fn new(mailmap: Arc<Mailmap>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            mailmap,
            rows: HashMap::new(),
        }))
    }

    /// `row` with its text, read now if no window handed it out yet.
    fn fill(&mut self, at: usize, row: &CommitRow, handle: Option<&RepoHandle>) -> CommitRow {
        let text = match self.rows.get(&at) {
            Some(text) => text.clone(),
            None => {
                let read = handle.map(|handle| handle.commit_text(&row.oid, &self.mailmap));
                match read {
                    Some(Ok(text)) => {
                        self.rows.insert(at, text.clone());
                        text
                    }
                    Some(Err(err)) => {
                        tracing::error!(error = ?err, context = "graph window: commit text");
                        CommitText::default()
                    }
                    None => CommitText::default(),
                }
            }
        };
        CommitRow {
            oid: row.oid.clone(),
            parents: row.parents.clone(),
            summary: text.summary,
            author_name: text.author_name,
            author_email: text.author_email,
            timestamp: row.timestamp,
            tz_offset_minutes: text.tz_offset_minutes,
        }
    }
}

/// A graph as windows read it: rows, the overlay's paint of them, the texts handed out.
#[derive(Debug, Clone)]
struct Shown {
    generation: u32,
    laid: Arc<Laid>,
    paint: Arc<Mutex<PaintMemo>>,
    /// `None` when the rows carry their text, as a filtered list's do.
    texts: Option<Arc<Mutex<Texts>>>,
}

#[derive(Debug)]
struct Graph {
    shown: Shown,
    query: CommitQuery,
    /// `None` when the refs could not be read; such a graph is never reused.
    refs: Option<u64>,
    /// The names a filtered list's rows were matched and read with.
    mailmap: Arc<Mailmap>,
    skipped: Vec<SkippedRef>,
    /// Every row is in.
    ended: bool,
    /// And so is the history: a walk may copy from this graph, a request be answered by it.
    complete: bool,
    /// The graph this walk replaces: still served by its generation until this one ends.
    base: Option<Shown>,
    /// Leading rows equal to `base`'s, and whether a row that differs has come yet.
    kept: u32,
    parted: bool,
    bytes: usize,
    used: u64,
}

impl Graph {
    fn total(&self) -> u32 {
        self.shown.laid.total()
    }

    /// The same rows: same query over the same refs. A filtered list matched its authors
    /// through the mailmap, so it needs that to be the same too.
    fn answers(&self, query: &CommitQuery, refs: Option<u64>, mailmap: &Arc<Mailmap>) -> bool {
        self.complete
            && refs.is_some()
            && self.refs == refs
            && self.query == *query
            && (self.shown.texts.is_some() || Arc::ptr_eq(&self.mailmap, mailmap))
    }

    /// What a new walk copies from: this graph once complete, else the one it replaces.
    fn reusable(&self) -> Option<Shown> {
        if self.complete {
            Some(self.shown.clone())
        } else {
            self.base.clone()
        }
    }

    /// Counts how far the rows from `from` on repeat `base`, up to the first that differs.
    fn compare(&mut self, from: usize) {
        let Some(base) = &self.base else {
            self.parted = true;
            return;
        };
        if self.parted {
            return;
        }
        let (old, new) = (&base.laid, &self.shown.laid);
        for at in from..new.rows.len() {
            let same = old.rows.get(at) == new.rows.get(at)
                && old.commits.get(at).map(|c| &c.oid) == new.commits.get(at).map(|c| &c.oid);
            if !same {
                self.parted = true;
                return;
            }
            self.kept += 1;
        }
    }

    fn progress(&self, is_last: bool) -> GraphProgress {
        GraphProgress {
            generation: self.shown.generation,
            total: self.total(),
            is_last,
            base: self.base.as_ref().map(|base| base.generation),
            kept: self.kept,
        }
    }
}

/// Blocks fetched of `base` stay good only if their names were read through this mailmap.
fn same_names(base: &Shown, mailmap: &Arc<Mailmap>) -> bool {
    base.texts
        .as_ref()
        .is_some_and(|texts| Arc::ptr_eq(&texts.lock().mailmap, mailmap))
}

#[derive(Debug, Default)]
pub(crate) struct GraphCache {
    graphs: HashMap<RepoId, Graph>,
    clock: u64,
}

impl GraphCache {
    /// The graph windows of `generation` read: the one on screen or the one it replaces.
    fn view(&self, repo: RepoId, generation: u32) -> Option<(&Shown, bool)> {
        let graph = self.graphs.get(&repo)?;
        if graph.shown.generation == generation {
            return Some((&graph.shown, graph.ended));
        }
        graph
            .base
            .as_ref()
            .filter(|base| base.generation == generation)
            .map(|base| (base, true))
    }

    fn held_mut(&mut self, repo: RepoId, generation: u32) -> Option<&mut Graph> {
        self.graphs
            .get_mut(&repo)
            .filter(|graph| graph.shown.generation == generation)
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    fn trim(&mut self, keep: RepoId, budget: usize) {
        let sizes: Vec<(RepoId, usize, u64)> = self
            .graphs
            .iter()
            .map(|(repo, graph)| (*repo, graph.bytes, graph.used))
            .collect();
        for repo in evicted(&sizes, keep, budget) {
            tracing::info!(repo = repo.0, "commit graph dropped from the cache");
            self.graphs.remove(&repo);
        }
    }
}

/// Least recently used first, until the rest fits in `budget`; `keep` is never chosen.
/// By bytes, not by count: one history can be a thousand times another.
fn evicted<K: Copy + Eq>(graphs: &[(K, usize, u64)], keep: K, budget: usize) -> Vec<K> {
    let mut total: usize = graphs.iter().map(|(_, bytes, _)| bytes).sum();
    let mut oldest: Vec<&(K, usize, u64)> =
        graphs.iter().filter(|(key, ..)| *key != keep).collect();
    oldest.sort_by_key(|(_, _, used)| *used);
    let mut out = Vec::new();
    for (key, bytes, _) in oldest {
        if total <= budget {
            break;
        }
        total -= bytes;
        out.push(*key);
    }
    out
}

fn commit_bytes(commit: &CommitRow) -> usize {
    size_of::<CommitRow>()
        + commit.oid.len()
        + commit.summary.len()
        + commit.author_name.len()
        + commit.author_email.len()
        + commit
            .parents
            .iter()
            .map(|parent| size_of::<String>() + parent.len())
            .sum::<usize>()
}

fn row_bytes(row: &GraphRow) -> usize {
    size_of::<GraphRow>() + row.segments.len() * size_of::<graph_engine::Segment>()
}

impl AppState {
    /// Walks and lays out the history into the cache; `on_progress` hears how many rows
    /// are ready after every chunk. A newer `begin_graph` ends the walk at its next chunk.
    /// The same query over the same refs is answered from the cache at once: commits never
    /// change, so only a moved ref can change the graph (R-300). Otherwise the walk takes
    /// time and parents of every commit the last graph of this repository holds and reads
    /// only the new ones (R-301).
    pub fn build_graph(
        &self,
        repo: RepoId,
        query: &CommitQuery,
        generation: u32,
        chunk_size: usize,
        mut on_progress: impl FnMut(GraphProgress) -> bool,
    ) -> Result<Vec<SkippedRef>, GitError> {
        let started = std::time::Instant::now();
        let handle = self.handle(repo)?;
        // Before the walk: a ref moving during it leaves an older print, never a newer one.
        let refs = handle
            .refs_fingerprint()
            .inspect_err(|err| tracing::error!(error = ?err, context = "graph cache: refs"))
            .ok();
        let mailmap = handle.mailmap();
        // A filtered list needs the text to match; a graph reads it by window (R-302).
        let lazy = !query.filters_rows();
        let base = {
            let mut cache = self.graph.write();
            let used = cache.tick();
            let mut base = None;
            if let Some(graph) = cache.graphs.get_mut(&repo) {
                // A walk that started late must not wipe the graph of the request after it.
                if generation < graph.shown.generation {
                    return Ok(Vec::new());
                }
                if graph.answers(query, refs, &mailmap) {
                    let mut progress = GraphProgress {
                        generation,
                        total: graph.total(),
                        is_last: true,
                        base: Some(graph.shown.generation),
                        kept: graph.total(),
                    };
                    // `.mailmap` changed: the layout stands, the names are read again.
                    if let Some(texts) = &graph.shown.texts
                        && !Arc::ptr_eq(&texts.lock().mailmap, &mailmap)
                    {
                        graph.shown.texts = Some(Texts::new(Arc::clone(&mailmap)));
                        progress.kept = 0;
                    }
                    graph.shown.generation = generation;
                    graph.used = used;
                    let skipped = graph.skipped.clone();
                    drop(cache);
                    tracing::info!(
                        repo = repo.0,
                        rows = progress.total,
                        "commit graph from the cache"
                    );
                    on_progress(progress);
                    return Ok(skipped);
                }
                base = graph.reusable();
            }
            // Retired already: a walk that would stop at its first chunk keeps the cache.
            if !self.is_current_graph(generation) {
                return Ok(Vec::new());
            }
            let parted = !lazy || !base.as_ref().is_some_and(|b| same_names(b, &mailmap));
            let graph = Graph {
                shown: Shown {
                    generation,
                    laid: Arc::default(),
                    paint: Arc::default(),
                    texts: lazy.then(|| Texts::new(Arc::clone(&mailmap))),
                },
                query: query.clone(),
                refs,
                mailmap,
                skipped: Vec::new(),
                ended: false,
                complete: false,
                base: base.clone(),
                kept: 0,
                parted,
                bytes: 0,
                used,
            };
            cache.graphs.insert(repo, graph);
            base
        };

        let mut record = WalkedHistory::default();
        let rows = GraphRows {
            reuse: base.as_ref().map(|base| Reuse {
                history: &base.laid.history,
            }),
            record: Some(&mut record),
            text: !lazy,
        };
        let on_chunk = |chunk: GraphChunk| {
            if !self.is_current_graph(generation) {
                return false;
            }
            let progress = {
                let mut cache = self.graph.write();
                let Some(graph) = cache.held_mut(repo, generation) else {
                    return false;
                };
                graph.ended |= chunk.is_last;
                // Rows lag commits by the long-link lookahead (R-330), so each is counted alone.
                graph.bytes += chunk.commits.iter().map(commit_bytes).sum::<usize>()
                    + chunk.rows.iter().map(row_bytes).sum::<usize>();
                let from = graph.shown.laid.rows.len();
                let laid = Arc::make_mut(&mut graph.shown.laid);
                laid.commits.extend(chunk.commits);
                laid.rows.extend(chunk.rows);
                laid.folds
                    .extend(chunk.folds.iter().map(|fold| (fold.row, fold.hidden)));
                graph.compare(from);
                graph.progress(chunk.is_last)
            };
            on_progress(progress)
        };
        let skipped = crate::graph_layout::lay_out(&handle, query, chunk_size, rows, on_chunk)?;

        let mut cache = self.graph.write();
        if let Some(graph) = cache.held_mut(repo, generation) {
            graph.skipped.clone_from(&skipped);
            if graph.ended {
                graph.complete = true;
                graph.bytes += record.bytes();
                Arc::make_mut(&mut graph.shown.laid).history = record;
                graph.base = None;
                tracing::info!(
                    repo = repo.0,
                    rows = graph.total(),
                    kept = graph.kept,
                    copied = base.is_some(),
                    elapsed_ms = started.elapsed().as_millis(),
                    "commit graph laid out"
                );
            }
        }
        cache.trim(repo, GRAPH_CACHE_BYTES);
        Ok(skipped)
    }

    /// Rows `start..start + count` of graph `generation`, cut short at what is laid out;
    /// `None` once a newer graph replaced it. A graph's rows get their subject and author
    /// here, read once per row (R-302).
    #[must_use]
    pub fn graph_window(
        &self,
        repo: RepoId,
        generation: u32,
        start: u32,
        count: u32,
    ) -> Option<GraphWindow> {
        let handle = self
            .handle(repo)
            .inspect_err(|err| tracing::error!(error = ?err, context = "graph window"))
            .ok();
        let cache = self.graph.read();
        let (shown, complete) = cache.view(repo, generation)?;
        let laid = &shown.laid;
        let len = laid.rows.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        let commits = match &shown.texts {
            Some(texts) => {
                let mut texts = texts.lock();
                (from..to)
                    .map(|at| texts.fill(at, &laid.commits[at], handle.as_ref()))
                    .collect()
            }
            None => laid.commits[from..to].to_vec(),
        };
        Some(GraphWindow {
            start,
            total: laid.total(),
            complete,
            commits,
            rows: laid.rows[from..to].to_vec(),
        })
    }

    /// The row of `oid` in graph `generation`. A scan: fifty thousand comparisons are well
    /// under a millisecond, and an index would cost every load instead of the rare lookup.
    #[must_use]
    pub fn graph_row_of(&self, repo: RepoId, generation: u32, oid: &str) -> Option<u32> {
        let cache = self.graph.read();
        let (shown, _) = cache.view(repo, generation)?;
        let row = shown
            .laid
            .commits
            .iter()
            .position(|commit| commit.oid == oid)?;
        // A commit walked but not laid out yet (R-330) has no row to scroll to.
        if row >= shown.laid.rows.len() {
            return None;
        }
        u32::try_from(row).ok()
    }

    /// The rows of graph `generation` and its paint memo, read under the cache's lock. The
    /// rows have their oid and parents; the text may not have been read yet (R-302).
    pub(crate) fn read_graph<R>(
        &self,
        repo: RepoId,
        generation: u32,
        read: impl FnOnce(&[CommitRow], &[GraphRow], &BTreeMap<u32, u32>, &Mutex<PaintMemo>) -> R,
    ) -> Option<R> {
        let cache = self.graph.read();
        let (shown, _) = cache.view(repo, generation)?;
        let laid = &shown.laid;
        Some(read(&laid.commits, &laid.rows, &laid.folds, &shown.paint))
    }

    pub(crate) fn forget_graph(&self, repo: RepoId) {
        self.graph.write().graphs.remove(&repo);
    }
}

#[cfg(test)]
mod tests {
    use super::evicted;

    #[test]
    fn nothing_goes_while_everything_fits() {
        assert!(evicted(&[(1, 10, 1), (2, 10, 2)], 2, 20).is_empty());
    }

    #[test]
    fn the_least_recently_used_go_first_until_the_rest_fits() {
        let graphs = [(1, 10, 3), (2, 10, 1), (3, 10, 2), (4, 10, 4)];
        assert_eq!(evicted(&graphs, 4, 20), vec![2, 3]);
    }

    #[test]
    fn the_graph_asked_for_stays_however_large() {
        assert_eq!(evicted(&[(1, 100, 2), (2, 5, 1)], 1, 20), vec![2]);
    }
}
