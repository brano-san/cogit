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
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

/// Rows the background reader reads between two looks at whether it should stop.
const PREFILL_BATCH: usize = 256;

/// Graphs stay until together they pass this; the one asked for last always stays. A
/// 50 000-commit history takes about 20 MB: four such repositories, or dozens of usual ones.
const GRAPH_CACHE_BYTES: usize = 96 << 20;

/// Between two progress reports after the first: often enough for the scrollbar, rare
/// enough that fifty thousand commits are a handful of messages, not 250.
pub const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(50);

/// `send` for the first report, the last and one per `every` between them; the rest wait
/// here, not in the IPC layer (INV-09, R-16). `false` from `send` stops the walk.
pub fn throttled(
    every: std::time::Duration,
    mut send: impl FnMut(GraphProgress) -> bool,
) -> impl FnMut(GraphProgress) -> bool {
    let mut last: Option<std::time::Instant> = None;
    move |progress| {
        let due = progress.is_last || last.is_none_or(|at| at.elapsed() >= every);
        if !due {
            return true;
        }
        last = Some(std::time::Instant::now());
        send(progress)
    }
}

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

/// Subject and author by commit, through one mailmap (R-390). Shared by the graphs of a
/// repository that read names the same way, so a rebuilt graph starts with them (R-303).
#[derive(Debug)]
struct Texts {
    mailmap: Arc<Mailmap>,
    by_oid: HashMap<String, CommitText>,
    bytes: usize,
    /// A background reader is at work on these.
    filling: bool,
}

fn text_bytes(oid: &str, text: &CommitText) -> usize {
    size_of::<(String, CommitText)>()
        + oid.len()
        + text.summary.len()
        + text.author_name.len()
        + text.author_email.len()
        + 16
}

impl Texts {
    fn new(mailmap: Arc<Mailmap>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            mailmap,
            by_oid: HashMap::new(),
            bytes: 0,
            filling: false,
        }))
    }

    fn insert(&mut self, oid: String, text: CommitText) {
        self.bytes += text_bytes(&oid, &text);
        self.by_oid.insert(oid, text);
    }

    /// `row` with its text, read now if nothing read it yet.
    fn fill(&mut self, row: &CommitRow, handle: Option<&RepoHandle>) -> CommitRow {
        let text = match self.by_oid.get(&row.oid) {
            Some(text) => text.clone(),
            None => {
                let read = handle.map(|handle| handle.commit_text(&row.oid, &self.mailmap));
                match read {
                    Some(Ok(text)) => {
                        self.insert(row.oid.clone(), text.clone());
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
    /// A filtered list keeps the graph shown before it, whole: clearing the filter goes
    /// back to it with its texts instead of walking and reading everything again.
    unfiltered: Option<Box<Graph>>,
}

impl Graph {
    fn total(&self) -> u32 {
        self.shown.laid.total()
    }

    /// What the cache's budget counts for this graph, and for the one it keeps.
    fn footprint(&self) -> usize {
        self.bytes
            + self.shown.texts.as_ref().map_or(0, |t| t.lock().bytes)
            + self.shown.paint.lock().bytes()
            + self
                .unfiltered
                .as_ref()
                .map_or(0, |graph| graph.footprint())
    }

    /// What a filtered list replacing this graph keeps for the way back.
    fn unfiltered(mut self) -> (Option<Box<Self>>, Option<Self>) {
        if self.complete && self.shown.texts.is_some() {
            (Some(Box::new(self)), None)
        } else {
            (self.unfiltered.take(), Some(self))
        }
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

/// What the background reader needs: weak, so a dropped graph ends it.
struct Prefill {
    laid: Weak<Laid>,
    texts: Weak<Mutex<Texts>>,
    walks: Arc<AtomicU64>,
    since: u64,
    /// Trimmed once the texts are in: they count toward its budget.
    cache: Weak<RwLock<GraphCache>>,
}

/// `None` when the rows carry their text or a reader is already on these texts.
fn prefill_of(
    shown: &Shown,
    walks: &Arc<AtomicU64>,
    cache: Weak<RwLock<GraphCache>>,
) -> Option<Prefill> {
    let texts = shown.texts.as_ref()?;
    {
        let mut texts = texts.lock();
        if texts.filling {
            return None;
        }
        texts.filling = true;
    }
    Some(Prefill {
        laid: Arc::downgrade(&shown.laid),
        texts: Arc::downgrade(texts),
        walks: Arc::clone(walks),
        since: walks.load(Ordering::SeqCst),
        cache,
    })
}

impl Prefill {
    fn run(self, root: &std::path::Path) {
        let started = std::time::Instant::now();
        let read = self.read(root);
        if let Some(texts) = self.texts.upgrade() {
            texts.lock().filling = false;
        }
        // The texts came after the walk that last trimmed the cache.
        if let Some(cache) = self.cache.upgrade() {
            let evicted = cache.write().trim_newest();
            drop(evicted);
        }
        match read {
            Ok(rows) => tracing::info!(
                rows,
                elapsed_ms = started.elapsed().as_millis(),
                "graph texts read ahead"
            ),
            Err(err) => tracing::error!(error = ?err, context = "graph text reader"),
        }
    }

    fn read(&self, root: &std::path::Path) -> Result<usize, GitError> {
        let handle = RepoHandle::open_root(root)?;
        let mut from = 0;
        let mut read = 0;
        loop {
            if self.walks.load(Ordering::SeqCst) != self.since {
                return Ok(read);
            }
            let (Some(laid), Some(texts)) = (self.laid.upgrade(), self.texts.upgrade()) else {
                return Ok(read);
            };
            let to = (from + PREFILL_BATCH).min(laid.commits.len());
            if from >= to {
                return Ok(read);
            }
            let (mailmap, wanted): (Arc<Mailmap>, Vec<&str>) = {
                let texts = texts.lock();
                let wanted = laid.commits[from..to]
                    .iter()
                    .map(|c| c.oid.as_str())
                    .filter(|oid| !texts.by_oid.contains_key(*oid))
                    .collect();
                (Arc::clone(&texts.mailmap), wanted)
            };
            let got: Vec<(String, CommitText)> = wanted
                .into_iter()
                .filter_map(|oid| {
                    handle
                        .commit_text(oid, &mailmap)
                        .inspect_err(|err| tracing::error!(error = ?err, context = "graph text"))
                        .ok()
                        .map(|text| (oid.to_owned(), text))
                })
                .collect();
            read += got.len();
            let mut texts = texts.lock();
            for (oid, text) in got {
                texts.insert(oid, text);
            }
            from = to;
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
    /// Bumped by every walk: a background reader stops rather than compete with one.
    walks: Arc<AtomicU64>,
    /// `GRAPH_CACHE_BYTES` unless a test set another.
    budget: Option<usize>,
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

    /// The graphs evicted, for the caller to [`release`] once the lock is let go.
    #[must_use]
    fn trim(&mut self, keep: RepoId) -> Vec<Graph> {
        let sizes: Vec<(RepoId, usize, u64)> = self
            .graphs
            .iter()
            .map(|(repo, graph)| (*repo, graph.footprint(), graph.used))
            .collect();
        evicted(&sizes, keep, self.budget.unwrap_or(GRAPH_CACHE_BYTES))
            .into_iter()
            .filter_map(|repo| {
                tracing::info!(repo = repo.0, "commit graph dropped from the cache");
                self.graphs.remove(&repo)
            })
            .collect()
    }

    /// `trim`, keeping the graph asked for last.
    #[must_use]
    fn trim_newest(&mut self) -> Vec<Graph> {
        let newest = self
            .graphs
            .iter()
            .max_by_key(|(_, graph)| graph.used)
            .map(|(repo, _)| *repo);
        newest.map_or_else(Vec::new, |repo| self.trim(repo))
    }
}

/// Frees what a graph held on a thread of its own, after the cache lock is let go: a large
/// graph is hundreds of thousands of allocations, and while they are freed under the lock
/// the windows and paint of every other repository wait, and so does the close.
fn release<T: Send + 'static>(gone: T) {
    let spawned = std::thread::Builder::new()
        .name("graph-free".to_owned())
        .spawn(move || drop(gone));
    if let Err(err) = spawned {
        tracing::error!(error = ?err, context = "graph cache: free off the lock");
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
            .refs_fingerprint(query.visible_refs.as_deref())
            .inspect_err(|err| tracing::error!(error = ?err, context = "graph cache: refs"))
            .ok();
        let mailmap = handle.mailmap();
        // A filtered list needs the text to match; a graph reads it by window (R-302).
        let lazy = !query.filters_rows();
        let (source, replaced) = {
            let mut cache = self.graph.write();
            let used = cache.tick();
            let walks = Arc::clone(&cache.walks);
            let mut base = None;
            let mut source = None;
            let mut list = None;
            if let Some(graph) = cache.graphs.get_mut(&repo) {
                // A walk that started late must not wipe the graph of the request after it.
                if generation < graph.shown.generation {
                    return Ok(Vec::new());
                }
                // Back from a filtered list to the graph shown before it.
                if lazy
                    && graph
                        .unfiltered
                        .as_ref()
                        .is_some_and(|unfiltered| unfiltered.answers(query, refs, &mailmap))
                    && let Some(unfiltered) = graph.unfiltered.take()
                {
                    list = Some(std::mem::replace(graph, *unfiltered));
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
                    let mut renamed = None;
                    if let Some(texts) = &graph.shown.texts
                        && !Arc::ptr_eq(&texts.lock().mailmap, &mailmap)
                    {
                        renamed = graph.shown.texts.replace(Texts::new(Arc::clone(&mailmap)));
                        progress.kept = 0;
                    }
                    graph.shown.generation = generation;
                    graph.used = used;
                    let skipped = graph.skipped.clone();
                    let prefill = prefill_of(&graph.shown, &walks, Arc::downgrade(&self.graph));
                    let evicted = cache.trim(repo);
                    drop(cache);
                    drop((renamed, list));
                    if !evicted.is_empty() {
                        release(evicted);
                    }
                    self.prefill(repo, prefill);
                    tracing::info!(
                        repo = repo.0,
                        rows = progress.total,
                        "commit graph from the cache"
                    );
                    on_progress(progress);
                    return Ok(skipped);
                }
                base = graph.reusable();
                // A filtered list copies little: the graph it keeps holds the whole history.
                source = graph
                    .unfiltered
                    .as_ref()
                    .and_then(|unfiltered| unfiltered.reusable())
                    .or_else(|| base.clone());
            }
            // Retired already: a walk that would stop at its first chunk keeps the cache.
            if !self.is_current_graph(generation) {
                return Ok(Vec::new());
            }
            let parted = !lazy || !base.as_ref().is_some_and(|b| same_names(b, &mailmap));
            let texts = lazy.then(|| {
                source
                    .as_ref()
                    .filter(|source| same_names(source, &mailmap))
                    .and_then(|source| source.texts.clone())
                    .unwrap_or_else(|| Texts::new(Arc::clone(&mailmap)))
            });
            cache.walks.fetch_add(1, Ordering::SeqCst);
            let mut graph = Graph {
                shown: Shown {
                    generation,
                    laid: Arc::default(),
                    paint: Arc::default(),
                    texts,
                },
                query: query.clone(),
                refs,
                mailmap,
                skipped: Vec::new(),
                ended: false,
                complete: false,
                base,
                kept: 0,
                parted,
                bytes: 0,
                used,
                unfiltered: None,
            };
            // Closed meanwhile: close has cleared the cache already, and nothing would again.
            if !self.repos.read().contains_key(&repo) {
                return Ok(Vec::new());
            }
            let mut replaced = cache.graphs.remove(&repo);
            if !lazy && let Some(old) = replaced.take() {
                (graph.unfiltered, replaced) = old.unfiltered();
            }
            cache.graphs.insert(repo, graph);
            (source, (replaced, list))
        };
        // Out of the lock: a graph cut short by a newer request held its rows alone.
        drop(replaced);

        let mut record = WalkedHistory::default();
        let rows = GraphRows {
            reuse: source.as_ref().map(|source| Reuse {
                history: &source.laid.history,
            }),
            record: Some(&mut record),
            text: !lazy,
            cut: None,
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
        let walks = Arc::clone(&cache.walks);
        let mut prefill = None;
        if let Some(graph) = cache.held_mut(repo, generation) {
            graph.skipped.clone_from(&skipped);
            if graph.ended {
                graph.complete = true;
                graph.bytes += record.bytes();
                Arc::make_mut(&mut graph.shown.laid).history = record;
                graph.base = None;
                prefill = prefill_of(&graph.shown, &walks, Arc::downgrade(&self.graph));
                tracing::info!(
                    repo = repo.0,
                    rows = graph.total(),
                    kept = graph.kept,
                    copied = source.is_some(),
                    elapsed_ms = started.elapsed().as_millis(),
                    "commit graph laid out"
                );
            }
        }
        let evicted = cache.trim(repo);
        drop(cache);
        if !evicted.is_empty() {
            release(evicted);
        }
        self.prefill(repo, prefill);
        Ok(skipped)
    }

    /// Reads the text of every row the graph has none for yet, top first, on a thread of
    /// its own: a jump through the list then finds it read (R-303). Stops when another walk
    /// starts or the graph is dropped; what it read stays.
    fn prefill(&self, repo: RepoId, prefill: Option<Prefill>) {
        let Some(prefill) = prefill else {
            return;
        };
        let Some(root) = self.get(repo).map(|open| open.root) else {
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("graph-text".to_owned())
            .spawn(move || prefill.run(&root));
        if let Err(err) = spawned {
            tracing::error!(error = ?err, context = "graph text reader");
        }
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
        // Out of the cache before any text is read from disk: a walk's next chunk waits on
        // the cache for writing, not on this window.
        let (laid, texts, complete) = {
            let cache = self.graph.read();
            let (shown, complete) = cache.view(repo, generation)?;
            (Arc::clone(&shown.laid), shown.texts.clone(), complete)
        };
        let len = laid.rows.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        let commits = match &texts {
            Some(texts) => {
                let mut texts = texts.lock();
                laid.commits[from..to]
                    .iter()
                    .map(|row| texts.fill(row, handle.as_ref()))
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

    /// How many commits of `repo` have their subject and author read: a count for tests.
    #[must_use]
    pub fn graph_texts_read(&self, repo: RepoId) -> usize {
        let cache = self.graph.read();
        cache
            .graphs
            .get(&repo)
            .and_then(|graph| graph.shown.texts.as_ref())
            .map_or(0, |texts| texts.lock().by_oid.len())
    }

    /// What the cache's budget counts for the graph of `repo`; 0 without one. For tests.
    #[must_use]
    pub fn graph_footprint(&self, repo: RepoId) -> usize {
        self.graph
            .read()
            .graphs
            .get(&repo)
            .map_or(0, Graph::footprint)
    }

    /// Another budget for the cache: filling the real one takes four large histories.
    pub fn set_graph_cache_budget(&self, bytes: usize) {
        self.graph.write().budget = Some(bytes);
    }

    pub(crate) fn forget_graph(&self, repo: RepoId) {
        let gone = self.graph.write().graphs.remove(&repo);
        if gone.is_some() {
            release(gone);
        }
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
