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
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphOverlay {
    pub start: u32,
    /// Rows laid out when this was painted: a later row can still change it.
    pub total: u32,
    pub node_lanes: Vec<u32>,
    /// `graph_engine::PAINT_SLOT` bits are the slot plus one; 0 is the default colour.
    pub node_styles: Vec<u8>,
    pub segment_first: Vec<u32>,
    pub segment_lanes: Vec<u32>,
    pub segment_styles: Vec<u8>,
}

/// The last paint, kept while neither the rows nor the request change.
#[derive(Debug, Default)]
pub(crate) struct PaintMemo {
    rows: usize,
    index: HashMap<String, u32>,
    parents: Vec<Vec<Option<u32>>>,
    request: Option<GraphPaintRequest>,
    paint: Paint,
}

fn row_index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl PaintMemo {
    fn refresh(&mut self, commits: &[CommitRow], rows: &[GraphRow], request: &GraphPaintRequest) {
        if self.rows != rows.len() {
            for (at, commit) in commits.iter().enumerate().skip(self.rows) {
                self.index.insert(commit.oid.clone(), row_index(at));
            }
            // A parent that arrived with this chunk resolves rows laid out before it.
            self.parents = commits
                .iter()
                .map(|c| {
                    c.parents
                        .iter()
                        .map(|p| self.index.get(p).copied())
                        .collect()
                })
                .collect();
            self.rows = rows.len();
            self.request = None;
        }
        if self.request.as_ref() == Some(request) {
            return;
        }
        let spec = PaintSpec {
            tips: request
                .tips
                .iter()
                .filter_map(|tip| self.index.get(&tip.oid).map(|row| (*row, tip.slot)))
                .collect(),
        };
        let watch = std::time::Instant::now();
        self.paint = graph_engine::paint(rows, &self.parents, &spec);
        tracing::debug!(
            rows = rows.len(),
            elapsed_us = watch.elapsed().as_micros(),
            "graph painted"
        );
        self.request = Some(request.clone());
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
        self.read_graph(repo, generation, |commits, rows, memo| {
            let mut memo = memo.lock();
            memo.refresh(commits, rows, request);
            memo.window(start, count)
        })
    }
}
