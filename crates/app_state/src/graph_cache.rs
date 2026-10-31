//! The laid-out graph stays on this side; the UI asks for the rows it shows (R-193). One
//! is kept per repository shown recently, so switching back is a lookup, not a walk (R-300);
//! a new walk takes time and parents from the last one instead of reading them (R-301); and
//! subject and author are read only for the rows a window hands out (R-302).

use crate::graph_overlay::PaintMemo;
use crate::{AppState, GraphChunk, RepoId};
use git_engine::{
    CommitQuery, CommitRow, CommitText, GitError, GraphRows, KnownCommits, Mailmap, ObjectId,
    ObjectMap, RepoHandle, Reuse, SkippedRef, WalkedHistory,
};
use graph_engine::GraphRow;
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

/// Rows the background reader reads between two looks at whether it should stop.
const PREFILL_BATCH: usize = 256;

/// Graphs stay until together they pass this; the one asked for last always stays. A
/// 50 000-commit history takes about 13 MB: seven such repositories, or dozens of usual ones.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphWindow {
    pub start: u32,
    pub total: u32,
    pub complete: bool,
    pub commits: Vec<CommitRow>,
    /// One per commit, in the same order.
    pub rows: Vec<GraphRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) id: ObjectId,
    pub(crate) time: i64,
    pub(crate) parents: Box<[ObjectId]>,
}

fn object_id(hex: &str) -> ObjectId {
    ObjectId::from_hex(hex.as_bytes()).unwrap_or_else(|err| {
        tracing::error!(error = ?err, context = "graph cache: object id");
        ObjectId::null(git_engine::HashKind::Sha1)
    })
}

pub(crate) fn take_text(commit: &mut CommitRow) -> CommitText {
    CommitText {
        summary: std::mem::take(&mut commit.summary),
        author_name: std::mem::take(&mut commit.author_name),
        author_email: std::mem::take(&mut commit.author_email),
        tz_offset_minutes: commit.tz_offset_minutes,
    }
}

impl Node {
    pub(crate) fn of(commit: &CommitRow) -> Self {
        Self {
            id: object_id(&commit.oid),
            time: commit.timestamp,
            parents: commit.parents.iter().map(|p| object_id(p)).collect(),
        }
    }

    fn row(&self, text: CommitText) -> CommitRow {
        CommitRow {
            oid: self.id.to_string(),
            parents: self.parents.iter().map(ToString::to_string).collect(),
            summary: text.summary,
            author_name: text.author_name,
            author_email: text.author_email,
            timestamp: self.time,
            tz_offset_minutes: text.tz_offset_minutes,
        }
    }

    fn bytes(&self) -> usize {
        size_of::<Self>() + self.parents.len() * size_of::<ObjectId>()
    }
}

#[derive(Debug)]
struct NodeIndex<'a> {
    nodes: &'a [Node],
    at: ObjectMap<ObjectId, u32>,
}

impl<'a> NodeIndex<'a> {
    fn new(nodes: &'a [Node]) -> Self {
        let mut at = ObjectMap::default();
        at.reserve(nodes.len());
        for (row, node) in nodes.iter().enumerate() {
            at.insert(node.id, u32::try_from(row).unwrap_or(u32::MAX));
        }
        Self { nodes, at }
    }
}

impl KnownCommits for NodeIndex<'_> {
    fn time_and_parents(&self, id: &ObjectId) -> Option<(i64, Vec<ObjectId>)> {
        let node = self.nodes.get(usize::try_from(*self.at.get(id)?).ok()?)?;
        Some((node.time, node.parents.to_vec()))
    }
}

#[derive(Debug, Default, Clone)]
struct Laid {
    commits: Vec<Node>,
    texts: Vec<CommitText>,
    rows: Vec<GraphRow>,
    /// Filled once the walk is over; what the next walk takes time and parents from.
    history: Option<WalkedHistory>,
    /// Merges folded by `collapse_merged` and how many commits each holds (#26).
    folds: BTreeMap<u32, u32>,
    /// A merge in progress: the Working Tree row, laid out with the commits (07 §4).
    working_tree: Option<GraphRow>,
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
    by_oid: ObjectMap<ObjectId, CommitText>,
    bytes: usize,
    /// A background reader is at work on these.
    filling: bool,
}

fn text_bytes(text: &CommitText) -> usize {
    size_of::<(ObjectId, CommitText)>()
        + 1
        + text.summary.len()
        + text.author_name.len()
        + text.author_email.len()
        + 16
}

impl Texts {
    fn new(mailmap: Arc<Mailmap>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            mailmap,
            by_oid: ObjectMap::default(),
            bytes: 0,
            filling: false,
        }))
    }

    fn insert(&mut self, oid: ObjectId, text: CommitText) {
        self.bytes += text_bytes(&text);
        self.by_oid.insert(oid, text);
    }

    /// `row` with its text, read now if nothing read it yet.
    fn fill(&mut self, node: &Node, handle: Option<&RepoHandle>) -> CommitRow {
        let text = match self.by_oid.get(&node.id) {
            Some(text) => text.clone(),
            None => {
                let read = handle.map(|handle| handle.commit_text_of(node.id, &self.mailmap));
                match read {
                    Some(Ok(text)) => {
                        self.insert(node.id, text.clone());
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
        node.row(text)
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
    replaced: Option<Shown>,
    /// What `base`'s rows count toward the budget; its texts and paint are counted live.
    base_bytes: usize,
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
            + self.base.as_ref().map_or(0, |base| {
                let texts = base
                    .texts
                    .as_ref()
                    .filter(|t| {
                        self.shown
                            .texts
                            .as_ref()
                            .is_none_or(|own| !Arc::ptr_eq(t, own))
                    })
                    .map_or(0, |t| t.lock().bytes);
                self.base_bytes + texts + base.paint.lock().bytes()
            })
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
                && old.commits.get(at).map(|c| c.id) == new.commits.get(at).map(|c| c.id);
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

    /// Shown again as `generation`; the texts it drops are for the caller to free off the lock.
    fn reissue(
        &mut self,
        generation: u32,
        used: u64,
        mailmap: &Arc<Mailmap>,
    ) -> (GraphProgress, Option<Arc<Mutex<Texts>>>) {
        let mut progress = GraphProgress {
            generation,
            total: self.total(),
            is_last: true,
            base: Some(self.shown.generation),
            kept: self.total(),
        };
        // `.mailmap` changed: the layout stands, the names are read again.
        let mut renamed = None;
        if let Some(texts) = &self.shown.texts
            && !Arc::ptr_eq(&texts.lock().mailmap, mailmap)
        {
            renamed = self.shown.texts.replace(Texts::new(Arc::clone(mailmap)));
            progress.kept = 0;
        }
        self.shown.generation = generation;
        self.used = used;
        (progress, renamed)
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
            let (mailmap, wanted): (Arc<Mailmap>, Vec<ObjectId>) = {
                let texts = texts.lock();
                let wanted = laid.commits[from..to]
                    .iter()
                    .map(|c| c.id)
                    .filter(|oid| !texts.by_oid.contains_key(oid))
                    .collect();
                (Arc::clone(&texts.mailmap), wanted)
            };
            let got: Vec<(ObjectId, CommitText)> = wanted
                .into_iter()
                .filter_map(|oid| {
                    handle
                        .commit_text_of(oid, &mailmap)
                        .inspect_err(|err| tracing::error!(error = ?err, context = "graph text"))
                        .ok()
                        .map(|text| (oid, text))
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
            .iter()
            .chain(&graph.replaced)
            .find(|base| base.generation == generation)
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

fn carried_bytes(text: &CommitText) -> usize {
    size_of::<CommitText>() + text.summary.len() + text.author_name.len() + text.author_email.len()
}

fn row_bytes(row: &GraphRow) -> usize {
    size_of::<GraphRow>()
        + row.segments.capacity() * size_of::<graph_engine::Segment>()
        + row
            .links
            .iter()
            .map(|link| size_of::<graph_engine::LongLink>() + link.oid.len())
            .sum::<usize>()
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
        if let Some(remote) = self.remote(repo) {
            return self.graph_through(&remote, query, generation, chunk_size, &mut on_progress);
        }
        let started = std::time::Instant::now();
        let handle = self.handle(repo)?;
        // Before the walk: a ref moving during it leaves an older print, never a newer one.
        let refs = handle
            .refs_fingerprint(query)
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
            let mut replaced_shown = None;
            let mut base_bytes = 0;
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
                    let (progress, renamed) = graph.reissue(generation, used, &mailmap);
                    let skipped = graph.skipped.clone();
                    let prefill = prefill_of(&graph.shown, &walks, Arc::downgrade(&self.graph));
                    self.settle(cache, repo, prefill, (renamed, list));
                    tracing::info!(
                        repo = repo.0,
                        rows = progress.total,
                        "commit graph from the cache"
                    );
                    on_progress(progress);
                    return Ok(skipped);
                }
                base = graph.reusable();
                replaced_shown = (!graph.complete).then(|| graph.shown.clone());
                base_bytes = if graph.complete {
                    graph.bytes
                } else {
                    graph.base_bytes
                };
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
                replaced: replaced_shown,
                base_bytes,
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

        let raw = lazy && !query.view.first_parent && !query.view.collapse_merged;
        let mut record = (!raw).then(WalkedHistory::default);
        let index = source
            .as_ref()
            .filter(|source| source.laid.history.is_none())
            .map(|source| NodeIndex::new(&source.laid.commits));
        let known: Option<&dyn KnownCommits> = match (&source, &index) {
            (_, Some(index)) => Some(index),
            (Some(source), None) => source
                .laid
                .history
                .as_ref()
                .map(|history| history as &dyn KnownCommits),
            (None, None) => None,
        };
        let rows = GraphRows {
            reuse: known.map(|history| Reuse { history }),
            record: record.as_mut(),
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
                graph.bytes += chunk.commits.iter().map(Node::bytes).sum::<usize>()
                    + chunk.texts.iter().map(carried_bytes).sum::<usize>()
                    + chunk.rows.iter().map(row_bytes).sum::<usize>();
                let from = graph.shown.laid.rows.len();
                let laid = Arc::make_mut(&mut graph.shown.laid);
                laid.commits.extend(chunk.commits);
                laid.texts.extend(chunk.texts);
                laid.rows.extend(chunk.rows);
                laid.folds
                    .extend(chunk.folds.iter().map(|fold| (fold.row, fold.hidden)));
                if chunk.working_tree.is_some() {
                    laid.working_tree = chunk.working_tree;
                }
                graph.compare(from);
                graph.progress(chunk.is_last)
            };
            on_progress(progress)
        };
        let skipped = crate::graph_layout::lay_out(&handle, query, chunk_size, rows, on_chunk)?;
        Ok(self.finish_walk(repo, generation, skipped, record, source.is_some(), started))
    }

    fn finish_walk(
        &self,
        repo: RepoId,
        generation: u32,
        skipped: Vec<SkippedRef>,
        record: Option<WalkedHistory>,
        copied: bool,
        started: std::time::Instant,
    ) -> Vec<SkippedRef> {
        let mut cache = self.graph.write();
        let walks = Arc::clone(&cache.walks);
        let mut prefill = None;
        if let Some(graph) = cache.held_mut(repo, generation) {
            graph.skipped.clone_from(&skipped);
            if graph.ended {
                graph.complete = true;
                if let Some(record) = record {
                    graph.bytes += record.bytes();
                    Arc::make_mut(&mut graph.shown.laid).history = Some(record);
                }
                graph.base = None;
                graph.replaced = None;
                graph.base_bytes = 0;
                prefill = prefill_of(&graph.shown, &walks, Arc::downgrade(&self.graph));
                tracing::info!(
                    repo = repo.0,
                    rows = graph.total(),
                    kept = graph.kept,
                    copied,
                    elapsed_ms = started.elapsed().as_millis(),
                    "commit graph laid out"
                );
            }
        }
        self.settle(cache, repo, prefill, ());
        skipped
    }

    /// Trims the cache; `gone` and the evicted graphs are freed after the guard, taken by value.
    fn settle<T>(
        &self,
        mut cache: RwLockWriteGuard<'_, GraphCache>,
        repo: RepoId,
        prefill: Option<Prefill>,
        gone: T,
    ) {
        let evicted = cache.trim(repo);
        drop(cache);
        drop(gone);
        if !evicted.is_empty() {
            release(evicted);
        }
        self.prefill(repo, prefill);
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
    pub(crate) fn local_graph_window(
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
        // The rows are copied under the lock and the texts read after it: a walk's next chunk
        // waits for neither, and no second `Arc<Laid>` makes its `make_mut` copy the graph.
        let (total, commits, carried, rows, texts, complete) = {
            let cache = self.graph.read();
            let (shown, complete) = cache.view(repo, generation)?;
            let laid = &shown.laid;
            let len = laid.rows.len();
            let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
            let to = from
                .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
                .min(len);
            (
                laid.total(),
                laid.commits[from..to].to_vec(),
                laid.texts.get(from..to).map(<[CommitText]>::to_vec),
                laid.rows[from..to].to_vec(),
                shown.texts.clone(),
                complete,
            )
        };
        let commits = match (&texts, carried) {
            (Some(texts), _) => {
                let mut texts = texts.lock();
                commits
                    .iter()
                    .map(|node| texts.fill(node, handle.as_ref()))
                    .collect()
            }
            (None, Some(carried)) => commits
                .iter()
                .zip(carried)
                .map(|(node, text)| node.row(text))
                .collect(),
            (None, None) => commits
                .iter()
                .map(|node| node.row(CommitText::default()))
                .collect(),
        };
        Some(GraphWindow {
            start,
            total,
            complete,
            commits,
            rows,
        })
    }

    /// [`Self::graph_window`] in the wire format (`graph_wire`) as base64: one string for
    /// the `postMessage` transport (R-192, R-194). Empty once a newer graph replaced it.
    #[must_use]
    pub fn graph_window_wire(
        &self,
        repo: RepoId,
        generation: u32,
        start: u32,
        count: u32,
    ) -> String {
        let window = self.graph_window(repo, generation, start, count);
        let bytes = window
            .map(|w| crate::graph_wire::encode(&w))
            .unwrap_or_default();
        diff_engine::base64(&bytes)
    }

    /// The Working Tree row of graph `generation` while a merge is in progress.
    #[must_use]
    pub(crate) fn local_graph_working_tree(
        &self,
        repo: RepoId,
        generation: u32,
    ) -> Option<GraphRow> {
        let cache = self.graph.read();
        let (shown, _) = cache.view(repo, generation)?;
        shown.laid.working_tree.clone()
    }

    /// The row of `oid` in graph `generation`. A scan: fifty thousand comparisons are well
    /// under a millisecond, and an index would cost every load instead of the rare lookup.
    #[must_use]
    pub(crate) fn local_graph_row_of(
        &self,
        repo: RepoId,
        generation: u32,
        oid: &str,
    ) -> Option<u32> {
        let id = ObjectId::from_hex(oid.as_bytes()).ok()?;
        let cache = self.graph.read();
        let (shown, _) = cache.view(repo, generation)?;
        let row = shown
            .laid
            .commits
            .iter()
            .position(|commit| commit.id == id)?;
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
        read: impl FnOnce(&[Node], &[GraphRow], &BTreeMap<u32, u32>, &Mutex<PaintMemo>, bool) -> R,
    ) -> Option<R> {
        let cache = self.graph.read();
        let (shown, complete) = cache.view(repo, generation)?;
        let laid = &shown.laid;
        Some(read(
            &laid.commits,
            &laid.rows,
            &laid.folds,
            &shown.paint,
            complete,
        ))
    }

    /// How many commits of `repo` have their subject and author read: a count for tests.
    #[must_use]
    pub(crate) fn local_graph_texts_read(&self, repo: RepoId) -> usize {
        let cache = self.graph.read();
        cache
            .graphs
            .get(&repo)
            .and_then(|graph| graph.shown.texts.as_ref())
            .map_or(0, |texts| texts.lock().by_oid.len())
    }

    /// What the cache's budget counts for the graph of `repo`; 0 without one. For tests.
    #[must_use]
    pub(crate) fn local_graph_footprint(&self, repo: RepoId) -> usize {
        self.graph
            .read()
            .graphs
            .get(&repo)
            .map_or(0, Graph::footprint)
    }

    /// Another budget for the cache: filling the real one takes four large histories.
    pub fn set_graph_cache_budget(&self, bytes: usize) {
        self.graph.write().budget = Some(bytes);
        self.budget_on_agents(bytes);
    }

    pub(crate) fn graph_cache_budget(&self) -> Option<usize> {
        self.graph.read().budget
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
    use crate::AppState;
    use git_engine::CommitQuery;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

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

    #[test]
    fn a_window_waiting_for_texts_holds_no_copy_of_the_graph() {
        let f = test_fixtures::linear(10).unwrap();
        let state = AppState::new();
        let repo = state.open_repository(f.path()).unwrap().repo;
        let generation = state.begin_graph();
        state
            .build_graph(repo, &CommitQuery::default(), generation, 4, |_| true)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.graph_texts_read(repo) < 10 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let (laid, texts) = {
            let cache = state.graph.read();
            let (shown, _) = cache.view(repo, generation).unwrap();
            (Arc::downgrade(&shown.laid), shown.texts.clone().unwrap())
        };
        let held = texts.lock();
        let before = Arc::strong_count(&texts);
        let window = std::thread::scope(|scope| {
            let window = scope.spawn(|| state.graph_window(repo, generation, 0, 5));
            // The window has taken its texts handle, so it is at, or on its way to, the lock.
            let until = Instant::now() + Duration::from_secs(5);
            while Arc::strong_count(&texts) == before && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(Arc::strong_count(&texts) > before);
            std::thread::sleep(Duration::from_millis(50));
            assert_eq!(laid.strong_count(), 1);
            drop(held);
            window.join().unwrap()
        });
        assert_eq!(window.unwrap().commits.len(), 5);
    }
}
