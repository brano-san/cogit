//! The laid-out graph stays on this side; the UI asks for the rows it shows (R-193). One
//! is kept per repository shown recently, so switching back is a lookup, not a walk (R-300).

use crate::{AppState, GraphChunk, RepoId};
use git_engine::{CommitQuery, CommitRow, GitError, SkippedRef};
use graph_engine::GraphRow;
use serde::Serialize;
use std::collections::HashMap;

/// Graphs stay until together they pass this; the one asked for last always stays. A
/// 50 000-commit history takes about 25 MB: four such repositories, or dozens of usual ones.
const GRAPH_CACHE_BYTES: usize = 96 << 20;

/// How far the walk got. The rows themselves travel only when asked for, by window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphProgress {
    pub generation: u32,
    /// Rows laid out so far: the list is this long while the rest is being walked.
    pub total: u32,
    pub is_last: bool,
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

#[derive(Debug)]
struct Graph {
    generation: u32,
    query: CommitQuery,
    /// `None` when the refs could not be read; such a graph is never reused.
    refs: Option<u64>,
    skipped: Vec<SkippedRef>,
    complete: bool,
    commits: Vec<CommitRow>,
    rows: Vec<GraphRow>,
    bytes: usize,
    used: u64,
}

impl Graph {
    const fn new(generation: u32, query: CommitQuery, refs: Option<u64>, used: u64) -> Self {
        Self {
            generation,
            query,
            refs,
            skipped: Vec::new(),
            complete: false,
            commits: Vec::new(),
            rows: Vec::new(),
            bytes: 0,
            used,
        }
    }

    fn total(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
    }

    fn answers(&self, query: &CommitQuery, refs: Option<u64>) -> bool {
        self.complete && refs.is_some() && self.refs == refs && self.query == *query
    }
}

#[derive(Debug, Default)]
pub(crate) struct GraphCache {
    graphs: HashMap<RepoId, Graph>,
    clock: u64,
}

impl GraphCache {
    fn held(&self, repo: RepoId, generation: u32) -> Option<&Graph> {
        self.graphs
            .get(&repo)
            .filter(|graph| graph.generation == generation)
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
    /// change, so only a moved ref can change the graph (R-300).
    pub fn build_graph(
        &self,
        repo: RepoId,
        query: &CommitQuery,
        generation: u32,
        chunk_size: usize,
        mut on_progress: impl FnMut(GraphProgress) -> bool,
    ) -> Result<Vec<SkippedRef>, GitError> {
        let handle = self.handle(repo)?;
        // Before the walk: a ref moving during it leaves an older print, never a newer one.
        let refs = handle
            .refs_fingerprint()
            .inspect_err(|err| tracing::error!(error = ?err, context = "graph cache: refs"))
            .ok();
        {
            let mut cache = self.graph.write();
            let used = cache.tick();
            if let Some(graph) = cache.graphs.get_mut(&repo) {
                // A walk that started late must not wipe the graph of the request after it.
                if generation < graph.generation {
                    return Ok(Vec::new());
                }
                if graph.answers(query, refs) {
                    graph.generation = generation;
                    graph.used = used;
                    let progress = GraphProgress {
                        generation,
                        total: graph.total(),
                        is_last: true,
                    };
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
            }
            // Retired already: a walk that would stop at its first chunk keeps the cache.
            if !self.is_current_graph(generation) {
                return Ok(Vec::new());
            }
            cache
                .graphs
                .insert(repo, Graph::new(generation, query.clone(), refs, used));
        }

        let skipped =
            crate::graph_layout::lay_out(&handle, query, chunk_size, |chunk: GraphChunk| {
                if !self.is_current_graph(generation) {
                    return false;
                }
                let total = {
                    let mut cache = self.graph.write();
                    let Some(graph) = cache.held_mut(repo, generation) else {
                        return false;
                    };
                    graph.complete |= chunk.is_last;
                    graph.bytes += chunk
                        .commits
                        .iter()
                        .zip(&chunk.rows)
                        .map(|(commit, row)| row_bytes(commit, row))
                        .sum::<usize>();
                    graph.commits.extend(chunk.commits);
                    graph.rows.extend(chunk.rows);
                    graph.total()
                };
                on_progress(GraphProgress {
                    generation,
                    total,
                    is_last: chunk.is_last,
                })
            })?;

        let mut cache = self.graph.write();
        if let Some(graph) = cache.held_mut(repo, generation) {
            graph.skipped.clone_from(&skipped);
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
        let graph = cache.held(repo, generation)?;
        let len = graph.rows.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        Some(GraphWindow {
            start,
            total: graph.total(),
            complete: graph.complete,
            commits: graph.commits[from..to].to_vec(),
            rows: graph.rows[from..to].to_vec(),
        })
    }

    /// The row of `oid` in graph `generation`. A scan: fifty thousand comparisons are well
    /// under a millisecond, and an index would cost every load instead of the rare lookup.
    #[must_use]
    pub fn graph_row_of(&self, repo: RepoId, generation: u32, oid: &str) -> Option<u32> {
        let cache = self.graph.read();
        let graph = cache.held(repo, generation)?;
        let row = graph.commits.iter().position(|commit| commit.oid == oid)?;
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
