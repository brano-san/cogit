mod lanes;

pub use lanes::layout;

use serde::Serialize;

const LANE_COLORS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitNode {
    pub oid: String,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Normal,
    Merge,
    Root,
    WorkingTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    Direct,
    Merge,
    Crossing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LaneAssignment {
    pub row: u32,
    pub lane: u16,
    pub color: u8,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from_row: u32,
    pub from_lane: u16,
    pub to_row: u32,
    pub to_lane: u16,
    pub color: u8,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphLayout {
    pub lanes: Vec<LaneAssignment>,
    pub edges: Vec<GraphEdge>,
    pub max_lane: u16,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutCursor {
    pub active: Vec<Option<String>>,
    pub lane_colors: Vec<u8>,
    pub origins: Vec<Vec<u16>>,
    pub next_color: u8,
    pub next_row: u32,
}

impl LayoutCursor {
    #[must_use]
    pub fn take_color(&mut self) -> u8 {
        let color = self.next_color % LANE_COLORS;
        self.next_color = self.next_color.wrapping_add(1);
        color
    }

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
