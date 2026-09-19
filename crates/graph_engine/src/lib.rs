//! Commit DAG topology and lane allocation.
//!
//! This crate deliberately has **no dependency on `gix`**: it takes plain
//! "oid + parents" pairs, which makes the whole algorithm testable with `insta`
//! snapshots and without creating a single repository.
//!
//! The algorithm is specified in `doc/07-graph-rendering.md` section 3 and is
//! implemented in M4.

use serde::Serialize;

/// Number of distinct lane colours. Chosen to stay distinguishable under the common
/// forms of colour blindness — see `doc/06-design-system.md`.
pub const LANE_COLORS: u8 = 8;

/// Input to the layout algorithm: one commit and its parents, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitNode {
    pub oid: String,
    pub parents: Vec<String>,
}

/// What kind of dot to draw for a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum NodeKind {
    Normal,
    Merge,
    Root,
    WorkingTree,
}

/// How an edge between two rows is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum EdgeKind {
    /// Straight continuation within one lane.
    Direct,
    /// Into a merge commit.
    Merge,
    /// Passing through without touching this row.
    Crossing,
}

/// Placement of one commit row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LaneAssignment {
    pub row: u32,
    pub lane: u16,
    pub color: u8,
    pub kind: NodeKind,
}

/// A curve between two rows, in row/lane space. Pixel coordinates are the frontend's job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GraphEdge {
    pub from_row: u32,
    pub from_lane: u16,
    pub to_row: u32,
    pub to_lane: u16,
    pub color: u8,
    pub kind: EdgeKind,
}

/// Layout of one chunk of history.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct GraphLayout {
    pub lanes: Vec<LaneAssignment>,
    pub edges: Vec<GraphEdge>,
    /// Widest lane index used, for sizing the graph gutter.
    pub max_lane: u16,
}

/// Carries lane occupancy and the colour counter between chunks.
///
/// Without this, a freshly loaded chunk would restart colour assignment and already
/// rendered branches would visibly change colour mid-scroll.
#[derive(Debug, Clone, Default)]
pub struct LayoutCursor {
    /// Which commit each lane is currently waiting for.
    pub active: Vec<Option<String>>,
    /// Colour of each active lane, parallel to `active`.
    pub lane_colors: Vec<u8>,
    /// Monotonic counter feeding new lane colours.
    pub next_color: u8,
    /// Index of the next row to emit, continuing across chunks.
    pub next_row: u32,
}

impl LayoutCursor {
    /// Picks the colour for a newly created lane and advances the counter.
    #[must_use]
    pub fn take_color(&mut self) -> u8 {
        let color = self.next_color % LANE_COLORS;
        self.next_color = self.next_color.wrapping_add(1);
        color
    }

    /// Index of the first free lane, or the position where a new one would be appended.
    #[must_use]
    pub fn first_free_lane(&self) -> usize {
        self.active
            .iter()
            .position(Option::is_none)
            .unwrap_or(self.active.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_colors_cycle_within_the_palette() {
        let mut cursor = LayoutCursor::default();
        let taken: Vec<u8> = (0..LANE_COLORS + 2).map(|_| cursor.take_color()).collect();
        assert_eq!(&taken[..8], &[0, 1, 2, 3, 4, 5, 6, 7]);
        // Wraps around rather than running off the end of the palette.
        assert_eq!(taken[8], 0);
        assert!(taken.iter().all(|c| *c < LANE_COLORS));
    }

    #[test]
    fn free_lane_is_reused_before_widening_the_graph() {
        let cursor = LayoutCursor {
            active: vec![Some("a".into()), None, Some("c".into())],
            ..Default::default()
        };
        assert_eq!(cursor.first_free_lane(), 1);
    }

    #[test]
    fn a_full_row_of_lanes_appends_a_new_one() {
        let cursor = LayoutCursor {
            active: vec![Some("a".into()), Some("b".into())],
            ..Default::default()
        };
        assert_eq!(cursor.first_free_lane(), 2);
    }
}
