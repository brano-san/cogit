//! The laid-out graph stays on this side; the UI asks for the rows it shows (R-193). One
//! is kept per repository shown recently, so switching back is a lookup, not a walk (R-300),
//! and a new walk copies the rows of the last one instead of reading them again (R-301).

use crate::{AppState, GraphChunk, RepoId};
use git_engine::{CommitQuery, CommitRow, GitError, Reuse, SkippedRef, WalkedHistory};
use graph_engine::GraphRow;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Graphs stay until together they pass this; the one asked for last always stays. A
/// 50 000-commit history takes about 30 MB: three such repositories, or dozens of usual ones.
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
    /// Filled once the walk is over; what the next walk copies rows from.
    history: WalkedHistory,
}

impl Laid {
    fn total(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
    }
}

#[derive(Debug)]
struct Graph {
    generation: u32,
    query: CommitQuery,
    /// `None` when the refs could not be read; such a graph is never reused.
    refs: Option<u64>,
    skipped: Vec<SkippedRef>,
    /// Every row is in.
    ended: bool,
    /// And so is the history: a walk may copy from this graph, a request be answered by it.
    complete: bool,
    laid: Arc<Laid>,
    /// The graph this walk replaces: still served by its generation until this one ends.
    base: Option<(u32, Arc<Laid>)>,
    /// Leading rows equal to `base`'s, and whether a row that differs has come yet.
    kept: u32,
    parted: bool,
    bytes: usize,
    used: u64,
}

impl Graph {
    fn total(&self) -> u32 {
        self.laid.total()
    }

    fn answers(&self, query: &CommitQuery, refs: Option<u64>) -> bool {
        self.complete && refs.is_some() && self.refs == refs && self.query == *query
    }

    /// What a new walk copies from: this graph once complete, else the one it replaces.
    fn reusable(&self) -> Option<(u32, Arc<Laid>)> {
        if self.complete {
            Some((self.generation, Arc::clone(&self.laid)))
        } else {
            self.base.clone()
        }
    }

    /// Counts how far the rows from `from` on repeat `base`, up to the first that differs.
    fn compare(&mut self, from: usize) {
        let Some((_, base)) = &self.base else {
            self.parted = true;
            return;
        };
        if self.parted {
            return;
        }
        let laid = &self.laid;
        for at in from..laid.rows.len() {
            let same = base.rows.get(at) == laid.rows.get(at)
                && base.commits.get(at).map(|c| &c.oid) == laid.commits.get(at).map(|c| &c.oid);
            if !same {
                self.parted = true;
                return;
            }
            self.kept += 1;
        }
    }

    fn progress(&self, is_last: bool) -> GraphProgress {
        GraphProgress {
            generation: self.generation,
            total: self.total(),
            is_last,
            base: self.base.as_ref().map(|(generation, _)| *generation),
            kept: self.kept,
        }
    }
}

/// A graph as a window reads it.
struct View<'a> {
    laid: &'a Laid,
    complete: bool,
}

#[derive(Debug, Default)]
pub(crate) struct GraphCache {
    graphs: HashMap<RepoId, Graph>,
    clock: u64,
}

impl GraphCache {
    fn view(&self, repo: RepoId, generation: u32) -> Option<View<'_>> {
        let graph = self.graphs.get(&repo)?;
        if graph.generation == generation {
            return Some(View {
                laid: &graph.laid,
                complete: graph.ended,
            });
        }
        match &graph.base {
            Some((base, laid)) if *base == generation => Some(View {
                laid,
                complete: true,
            }),
            _ => None,
        }
    }

    fn held_mut(&mut self, repo: RepoId, generation: u32) -> Option<&mut Graph> {
        self.graphs
            .get_mut(&repo)
            .filter(|graph| graph.generation == generation)
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

fn row_bytes(commit: &CommitRow, row: &GraphRow) -> usize {
    size_of::<CommitRow>()
        + size_of::<GraphRow>()
        + commit.oid.len()
        + commit.summary.len()
        + commit.author_name.len()
        + commit.author_email.len()
        + commit
            .parents
            .iter()
            .map(|parent| size_of::<String>() + parent.len())
            .sum::<usize>()
        + row.segments.len() * size_of::<graph_engine::Segment>()
}

impl AppState {
    /// Walks and lays out the history into the cache; `on_progress` hears how many rows
    /// are ready after every chunk. A newer `begin_graph` ends the walk at its next chunk.
    /// The same query over the same refs is answered from the cache at once: commits never
    /// change, so only a moved ref can change the graph (R-300). Otherwise the walk copies
    /// every commit the last graph of this repository holds and reads only the new ones.
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
        let base = {
            let mut cache = self.graph.write();
            let used = cache.tick();
            let mut base = None;
            if let Some(graph) = cache.graphs.get_mut(&repo) {
                // A walk that started late must not wipe the graph of the request after it.
                if generation < graph.generation {
                    return Ok(Vec::new());
                }
                if graph.answers(query, refs) {
                    let progress = GraphProgress {
                        generation,
                        total: graph.total(),
                        is_last: true,
                        base: Some(graph.generation),
                        kept: graph.total(),
                    };
                    graph.generation = generation;
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
            let graph = Graph {
                generation,
                query: query.clone(),
                refs,
                skipped: Vec::new(),
                ended: false,
                complete: false,
                laid: Arc::default(),
                base: base.clone(),
                kept: 0,
                parted: false,
                bytes: 0,
                used,
            };
            cache.graphs.insert(repo, graph);
            base
        };

        let reuse = base.as_ref().map(|(_, laid)| Reuse {
            history: &laid.history,
            rows: &laid.commits,
        });
        let mut record = WalkedHistory::default();
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
                graph.bytes += chunk
                    .commits
                    .iter()
                    .zip(&chunk.rows)
                    .map(|(commit, row)| row_bytes(commit, row))
                    .sum::<usize>();
                let from = graph.laid.rows.len();
                let laid = Arc::make_mut(&mut graph.laid);
                laid.commits.extend(chunk.commits);
                laid.rows.extend(chunk.rows);
                graph.compare(from);
                graph.progress(chunk.is_last)
            };
            on_progress(progress)
        };
        let skipped = crate::graph_layout::lay_out(
            &handle,
            query,
            chunk_size,
            reuse,
            Some(&mut record),
            on_chunk,
        )?;

        let mut cache = self.graph.write();
        if let Some(graph) = cache.held_mut(repo, generation) {
            graph.skipped.clone_from(&skipped);
            if graph.ended {
                graph.complete = true;
                graph.bytes += record.bytes();
                Arc::make_mut(&mut graph.laid).history = record;
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
    /// `None` once a newer graph replaced it.
    #[must_use]
    pub fn graph_window(
        &self,
        repo: RepoId,
        generation: u32,
        start: u32,
        count: u32,
    ) -> Option<GraphWindow> {
        let cache = self.graph.read();
        let View { laid, complete } = cache.view(repo, generation)?;
        let len = laid.rows.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        Some(GraphWindow {
            start,
            total: laid.total(),
            complete,
            commits: laid.commits[from..to].to_vec(),
            rows: laid.rows[from..to].to_vec(),
        })
    }

    /// The row of `oid` in graph `generation`. A scan: fifty thousand comparisons are well
    /// under a millisecond, and an index would cost every load instead of the rare lookup.
    #[must_use]
    pub fn graph_row_of(&self, repo: RepoId, generation: u32, oid: &str) -> Option<u32> {
        let cache = self.graph.read();
        let view = cache.view(repo, generation)?;
        let row = view
            .laid
            .commits
            .iter()
            .position(|commit| commit.oid == oid)?;
        u32::try_from(row).ok()
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
