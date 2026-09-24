//! The laid-out graph stays on this side; the UI asks for the rows it shows (R-193).

use crate::{AppState, GraphChunk, RepoId};
use git_engine::{CommitQuery, CommitRow, GitError, SkippedRef};
use graph_engine::GraphRow;
use serde::Serialize;

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

/// One graph is on screen at a time, so one is kept.
#[derive(Debug, Default)]
pub(crate) struct GraphCache {
    repo: Option<RepoId>,
    generation: u32,
    commits: Vec<CommitRow>,
    rows: Vec<GraphRow>,
    complete: bool,
}

impl GraphCache {
    fn holds(&self, repo: RepoId, generation: u32) -> bool {
        self.repo == Some(repo) && self.generation == generation
    }

    fn total(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
    }
}

impl AppState {
    /// Walks and lays out the history into the cache; `on_progress` hears how many rows
    /// are ready after every chunk. A newer `begin_graph` ends the walk at its next chunk.
    pub fn build_graph(
        &self,
        repo: RepoId,
        query: &CommitQuery,
        generation: u32,
        chunk_size: usize,
        mut on_progress: impl FnMut(GraphProgress) -> bool,
    ) -> Result<Vec<SkippedRef>, GitError> {
        {
            let mut cache = self.graph.write();
            // A walk that started late must not wipe the graph of the request after it.
            if cache.repo.is_some() && generation < cache.generation {
                return Ok(Vec::new());
            }
            *cache = GraphCache {
                repo: Some(repo),
                generation,
                ..GraphCache::default()
            };
        }

        self.search_graph(repo, query, chunk_size, |chunk: GraphChunk| {
            if !self.is_current_graph(generation) {
                return false;
            }
            let total = {
                let mut cache = self.graph.write();
                if !cache.holds(repo, generation) {
                    return false;
                }
                cache.complete |= chunk.is_last;
                cache.commits.extend(chunk.commits);
                cache.rows.extend(chunk.rows);
                cache.total()
            };
            on_progress(GraphProgress {
                generation,
                total,
                is_last: chunk.is_last,
            })
        })
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
        if !cache.holds(repo, generation) {
            return None;
        }
        let len = cache.rows.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        Some(GraphWindow {
            start,
            total: cache.total(),
            complete: cache.complete,
            commits: cache.commits[from..to].to_vec(),
            rows: cache.rows[from..to].to_vec(),
        })
    }

    /// The row of `oid` in graph `generation`. A scan: fifty thousand comparisons are well
    /// under a millisecond, and an index would cost every load instead of the rare lookup.
    #[must_use]
    pub fn graph_row_of(&self, repo: RepoId, generation: u32, oid: &str) -> Option<u32> {
        let cache = self.graph.read();
        if !cache.holds(repo, generation) {
            return None;
        }
        let row = cache.commits.iter().position(|commit| commit.oid == oid)?;
        // A commit walked but not laid out yet (R-330) has no row to scroll to.
        if row >= cache.rows.len() {
            return None;
        }
        u32::try_from(row).ok()
    }

    pub(crate) fn forget_graph(&self, repo: RepoId) {
        let mut cache = self.graph.write();
        if cache.repo == Some(repo) {
            *cache = GraphCache::default();
        }
    }
}
