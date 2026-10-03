//! `graph_engine::paint` over the cached graph, handed out by window like the rows.

use std::collections::HashMap;

use crate::{AppState, RepoId};
use git_engine::CommitRow;
use graph_engine::{GraphRow, Paint, PaintSpec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PaintTip {
    pub oid: String,
    pub slot: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphPaintRequest {
    #[serde(default)]
    pub tips: Vec<PaintTip>,
    /// All but this commit's ancestors and descendants is dimmed.
    #[serde(default)]
    pub ancestry_of: Option<String>,
    /// Mergeable Coloring: HEAD's history is dimmed, what it has not merged stands out.
    #[serde(default)]
    pub mergeable: bool,
    /// Branch Coloring: merged-in lines are dimmed.
    #[serde(default)]
    pub dim_merges: bool,
    /// Varying Coloring: every branch in a colour of its own, kept through its merge.
    #[serde(default)]
    pub varying: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphOverlay {
    pub start: u32,
    /// Rows laid out when this was painted: a later row can still change it.
    pub total: u32,
    pub node_lanes: Vec<u32>,
    /// `graph_engine::PAINT_SLOT` bits are the slot plus one, 0 the default colour; `PAINT_DIM` dims.
    pub node_styles: Vec<u8>,
    pub segment_first: Vec<u32>,
    pub segment_lanes: Vec<u32>,
    pub segment_styles: Vec<u8>,
    /// Folded merges among the window's rows.
    pub folds: Vec<graph_engine::Fold>,
}

/// The last paint, kept while neither the rows nor the request change.
#[derive(Debug, Default)]
pub(crate) struct PaintMemo {
    rows: usize,
    index: HashMap<String, u32>,
    parents: Vec<Vec<Option<u32>>>,
    /// Parents not seen yet, with the commit and slot of each that waits for it.
    pending: HashMap<String, Vec<(usize, usize)>>,
    /// The rows changed since `paint`.
    stale: bool,
    painted: Option<std::time::Instant>,
    cost: std::time::Duration,
    request: Option<GraphPaintRequest>,
    paint: Paint,
    /// What `index` and `parents` hold, kept as they grow: the cache counts it (R-300).
    index_bytes: usize,
    parents_bytes: usize,
    /// `pending` too: parents a shallow or filtered history never shows wait there for good.
    pending_bytes: usize,
}

/// What `oid` waited for by `waiting` commits costs in `PaintMemo::pending`; none costs nothing.
fn pending_cost(oid: &str, waiting: usize) -> usize {
    if waiting == 0 {
        return 0;
    }
    size_of::<(String, Vec<(usize, usize)>)>() + oid.len() + waiting * size_of::<(usize, usize)>()
}

fn row_index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl PaintMemo {
    fn refresh(
        &mut self,
        commits: &[CommitRow],
        rows: &[GraphRow],
        request: &GraphPaintRequest,
        complete: bool,
    ) {
        if self.rows != rows.len() {
            let known = self.parents.len();
            for (at, commit) in commits.iter().enumerate().skip(known) {
                if self
                    .index
                    .insert(commit.oid.clone(), row_index(at))
                    .is_none()
                {
                    self.index_bytes += size_of::<(String, u32)>() + commit.oid.len();
                }
                // A parent that arrived with this chunk resolves rows laid out before it.
                if let Some(waiting) = self.pending.remove(&commit.oid) {
                    self.pending_bytes -= pending_cost(&commit.oid, waiting.len());
                    for (child, slot) in waiting {
                        self.parents[child][slot] = Some(row_index(at));
                    }
                }
            }
            for (at, commit) in commits.iter().enumerate().skip(known) {
                let resolved = commit
                    .parents
                    .iter()
                    .enumerate()
                    .map(|(slot, parent)| {
                        let row = self.index.get(parent).copied();
                        if row.is_none() {
                            let waiting = self.pending.entry(parent.clone()).or_default();
                            self.pending_bytes -= pending_cost(parent, waiting.len());
                            waiting.push((at, slot));
                            self.pending_bytes += pending_cost(parent, waiting.len());
                        }
                        row
                    })
                    .collect();
                self.parents.push(resolved);
                self.parents_bytes +=
                    size_of::<Vec<Option<u32>>>() + commit.parents.len() * size_of::<Option<u32>>();
            }
            self.rows = rows.len();
            self.stale = true;
        }
        if !self.stale && self.request.as_ref() == Some(request) {
            return;
        }
        // While the walk goes on the rows change at every request and a repaint costs the
        // whole graph: none is taken before five times the last one has gone by.
        if !complete
            && self.request.as_ref() == Some(request)
            && self.painted.is_some_and(|at| at.elapsed() < self.cost * 5)
        {
            return;
        }
        let spec = PaintSpec {
            tips: request
                .tips
                .iter()
                .filter_map(|tip| self.index.get(&tip.oid).map(|row| (*row, tip.slot)))
                .collect(),
            ancestry_of: request
                .ancestry_of
                .as_ref()
                .and_then(|oid| self.index.get(oid).copied()),
            mergeable: request.mergeable,
            dim_merges: request.dim_merges,
            varying: request.varying,
        };
        let watch = std::time::Instant::now();
        let row_of = |oid: &str| self.index.get(oid).copied();
        self.paint = graph_engine::paint(rows, &self.parents, &row_of, &spec);
        self.cost = watch.elapsed();
        self.painted = Some(std::time::Instant::now());
        self.stale = false;
        tracing::debug!(
            rows = rows.len(),
            elapsed_us = watch.elapsed().as_micros(),
            "graph painted"
        );
        self.request = Some(request.clone());
    }

    /// Roughly what the memo holds, for a cache that counts bytes.
    pub(crate) fn bytes(&self) -> usize {
        let paint = &self.paint;
        let lanes = paint.node_lane.len() + paint.segment_first.len() + paint.segment_lane.len();
        let styles = paint.node_style.len() + paint.segment_style.len();
        self.index_bytes
            + self.parents_bytes
            + self.pending_bytes
            + lanes * size_of::<u32>()
            + styles
    }

    fn window(&self, start: u32, count: u32) -> GraphOverlay {
        let len = self.paint.node_lane.len();
        let from = usize::try_from(start).unwrap_or(usize::MAX).min(len);
        let to = from
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(len);
        let first = self.paint.segment_first.get(from).copied().unwrap_or(0) as usize;
        let last = self.paint.segment_first.get(to).copied().unwrap_or(0) as usize;
        let offset = u32::try_from(first).unwrap_or(u32::MAX);
        GraphOverlay {
            start,
            total: row_index(len),
            node_lanes: self.paint.node_lane[from..to].to_vec(),
            node_styles: self.paint.node_style[from..to].to_vec(),
            segment_first: self
                .paint
                .segment_first
                .get(from..=to)
                .unwrap_or_default()
                .iter()
                .map(|at| at - offset)
                .collect(),
            segment_lanes: self.paint.segment_lane[first..last].to_vec(),
            segment_styles: self.paint.segment_style[first..last].to_vec(),
            folds: Vec::new(),
        }
    }
}

impl AppState {
    /// `None` once a newer graph replaced `generation`.
    #[must_use]
    pub fn graph_overlay(
        &self,
        repo: RepoId,
        generation: u32,
        start: u32,
        count: u32,
        request: &GraphPaintRequest,
    ) -> Option<GraphOverlay> {
        self.read_graph(repo, generation, |commits, rows, folds, memo, complete| {
            let mut memo = memo.lock();
            memo.refresh(commits, rows, request, complete);
            let mut window = memo.window(start, count);
            window.folds = folds
                .range(start..start.saturating_add(count))
                .map(|(&row, &hidden)| graph_engine::Fold { row, hidden })
                .collect();
            window
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_engine::{CommitNode, LayoutCursor};
    use std::time::{Duration, Instant};

    /// A merge of two branches over a root, newest first: `m`, `a`, `b`, `r`.
    fn history() -> (Vec<CommitRow>, Vec<GraphRow>) {
        let shape = [
            ("m", &["a", "b"][..]),
            ("a", &["r"]),
            ("b", &["r"]),
            ("r", &[]),
        ];
        let commits: Vec<CommitRow> = shape
            .iter()
            .map(|(oid, parents)| CommitRow {
                oid: (*oid).to_owned(),
                parents: parents.iter().map(|p| (*p).to_owned()).collect(),
                summary: String::new(),
                author_name: String::new(),
                author_email: String::new(),
                timestamp: 0,
                tz_offset_minutes: 0,
            })
            .collect();
        let nodes: Vec<CommitNode> = commits
            .iter()
            .map(|c| CommitNode {
                oid: c.oid.clone(),
                parents: c.parents.clone(),
                hidden: Vec::new(),
            })
            .collect();
        let rows = graph_engine::layout(&nodes, &mut LayoutCursor::default());
        (commits, rows)
    }

    fn request() -> GraphPaintRequest {
        GraphPaintRequest {
            tips: vec![PaintTip {
                oid: "m".to_owned(),
                slot: 3,
            }],
            ..GraphPaintRequest::default()
        }
    }

    #[test]
    fn a_graph_given_in_chunks_paints_as_the_whole_graph_does() {
        let (commits, rows) = history();
        let mut chunked = PaintMemo::default();
        chunked.refresh(&commits[..2], &rows[..2], &request(), true);
        chunked.refresh(&commits[..3], &rows[..3], &request(), true);
        chunked.refresh(&commits, &rows, &request(), true);
        let mut whole = PaintMemo::default();
        whole.refresh(&commits, &rows, &request(), true);

        assert_eq!(chunked.parents, whole.parents);
        assert_eq!(chunked.paint, whole.paint);
        assert!(chunked.pending.is_empty());
        assert_eq!(chunked.bytes(), whole.bytes());
    }

    // A parent the walk never shows (a shallow cut, a filter) waits in `pending` for as long as
    // the graph lives: the cache that counts bytes has to see it.
    #[test]
    fn parents_still_waited_for_are_counted_in_the_budget() {
        let (commits, rows) = history();
        let mut memo = PaintMemo::default();
        memo.refresh(&commits[..2], &rows[..2], &request(), true);
        assert!(
            !memo.pending.is_empty(),
            "the fixture must leave a parent out"
        );
        let floor: usize = memo
            .pending
            .iter()
            .map(|(oid, w)| pending_cost(oid, w.len()))
            .sum();
        assert!(
            floor > 0 && memo.bytes() >= floor,
            "{} < {floor}",
            memo.bytes()
        );
        assert_eq!(memo.pending_bytes, floor);

        memo.refresh(&commits, &rows, &request(), true);
        assert!(memo.pending.is_empty());
        assert_eq!(memo.pending_bytes, 0);
    }

    #[test]
    fn a_repaint_during_the_walk_waits_five_times_the_last_one() {
        let (commits, rows) = history();
        let mut memo = PaintMemo::default();
        memo.refresh(&commits[..2], &rows[..2], &request(), false);
        memo.cost = Duration::from_secs(3600);
        memo.painted = Some(Instant::now());

        memo.refresh(&commits, &rows, &request(), false);
        assert_eq!(memo.window(0, 10).total, 2);

        memo.refresh(&commits, &rows, &request(), true);
        assert_eq!(memo.window(0, 10).total, 4);
    }
}
