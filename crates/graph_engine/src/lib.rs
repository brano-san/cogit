mod lanes;
mod paint;
mod view;

pub use lanes::{finish, layout, pass_through, push};
pub use paint::{PAINT_DIM, PAINT_SLOT, Paint, PaintSpec, paint};
pub use view::{Fold, ViewFilter};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

const LANE_COLORS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitNode {
    pub oid: String,
    pub parents: Vec<String>,
    /// Parents the list does not show; their line ends in an arrow under the node.
    pub hidden: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Normal,
    Merge,
    Root,
    WorkingTree,
}

/// The part of its row a segment covers: the upper half, the lower half or all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Span {
    Top,
    Bottom,
    Through,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub from: u16,
    pub to: u16,
    pub span: Span,
    pub primary: bool,
    pub color: u8,
    pub arrow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub row: u32,
    pub lane: u16,
    pub color: u8,
    pub kind: NodeKind,
    pub primary: bool,
    /// Columns used by the top edge, the node and the bottom edge together.
    pub width: u16,
    pub segments: Vec<Segment>,
    /// Stubs standing for a link too long to draw whole (R-330), with the far end of each.
    pub links: Vec<LongLink>,
}

/// `segments[segment]` is one stub of a cut link; `oid` is the commit at its other end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LongLink {
    pub segment: u16,
    pub oid: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Above {
    pub(crate) id: u64,
    pub(crate) color: u8,
    pub(crate) primary: bool,
}

/// A line on its way down to the commit it waits for. Lines sit side by side without
/// holes: a column is a line's for one gap between two rows, not for its whole life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lane {
    pub(crate) id: u64,
    pub(crate) waits: String,
    pub(crate) color: u8,
    pub(crate) primary: bool,
    /// The row of the commit it waits for, `u32::MAX` past the lookahead.
    pub(crate) ends: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutCursor {
    pub(crate) lanes: Vec<Lane>,
    pub(crate) next_id: u64,
    pub(crate) next_color: u8,
    pub(crate) next_row: u32,
    /// The primary ref's commit: its lane turns primary there, wherever the layout put it.
    pub(crate) mainline: Option<String>,
    pub(crate) above: Vec<Above>,
    pub(crate) converging: Vec<u64>,
    pub(crate) leaving: Vec<u64>,
    pub(crate) middle: Vec<u64>,
    /// Links longer than this many rows are cut into two stubs; 0 draws every link whole.
    pub(crate) long_links: usize,
    /// Commits held back before they are placed: how long a new line is, is known within them.
    pub(crate) lookahead: usize,
    /// The next `lookahead` commits, not placed yet, with the row each will get.
    pub(crate) pending: VecDeque<CommitNode>,
    pub(crate) ahead: HashMap<String, u32>,
    /// A parent whose links were cut, and the children at their upper ends.
    pub(crate) cut_into: HashMap<String, Vec<String>>,
    /// A parent whose first-parent link was cut, and the colour of the child: the line goes
    /// on in it below the arrow.
    pub(crate) cut_colour: HashMap<String, u8>,
}

impl LayoutCursor {
    /// The main line starts at `tip` and follows its first parents (R-115); it only marks
    /// lines, the columns are the top commit's (R-783).
    #[must_use]
    pub fn with_mainline(tip: Option<String>) -> Self {
        Self {
            mainline: tip,
            ..Self::default()
        }
    }

    /// Cuts links longer than `rows` rows (R-330); rows then come out `rows` commits late.
    #[must_use]
    pub fn with_long_links(mut self, rows: u32) -> Self {
        self.long_links = usize::try_from(rows).unwrap_or(usize::MAX);
        self.lookahead = self.lookahead.max(self.long_links);
        self
    }

    /// Holds `rows` commits back so a new line knows how long it is (07 §3); rows then come
    /// out that many commits late. Without it every new line counts as long.
    #[must_use]
    pub fn with_lookahead(mut self, rows: u32) -> Self {
        self.lookahead = usize::try_from(rows)
            .unwrap_or(usize::MAX)
            .max(self.long_links);
        self
    }

    pub(crate) fn take_color(&mut self) -> u8 {
        let color = self.next_color % LANE_COLORS;
        self.next_color = self.next_color.wrapping_add(1);
        color
    }

    pub(crate) fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// HEAD's line, failing that `master`, then `main` (R-115).
#[must_use]
pub fn mainline_tip(local_branches: &[(&str, &str)], head_oid: Option<&str>) -> Option<String> {
    if let Some(oid) = head_oid {
        return Some(oid.to_owned());
    }
    for wanted in ["master", "main"] {
        if let Some((_, oid)) = local_branches.iter().find(|(name, _)| *name == wanted) {
            return Some((*oid).to_owned());
        }
    }
    None
}
