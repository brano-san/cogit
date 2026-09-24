//! Colour over a finished layout; the layout itself never changes for it (R-340).

use crate::{GraphRow, Segment, Span};

pub const PAINT_SLOT: u8 = 0x0f;

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaintSpec {
    /// Tips by row with their palette slot; a tip higher up claims its line first.
    pub tips: Vec<(u32, u8)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Paint {
    pub node_lane: Vec<u32>,
    pub node_style: Vec<u8>,
    /// Where each row's segments start in `segment_lane` and `segment_style`, then the end.
    pub segment_first: Vec<u32>,
    pub segment_lane: Vec<u32>,
    pub segment_style: Vec<u8>,
}

/// A stretch of lane between two nodes: the commit it leaves and the node it ends in.
#[derive(Debug, Clone, Copy)]
struct Group {
    lane: u32,
    child: u32,
    own: bool,
    end: u32,
}

#[derive(Debug, Default)]
struct Trace {
    groups: Vec<Group>,
    joins: Vec<(u32, u32)>,
    node_lane: Vec<u32>,
    segment_first: Vec<u32>,
    segment_group: Vec<u32>,
}

fn index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(NONE)
}

fn at(columns: &[u32], column: u16) -> u32 {
    columns.get(usize::from(column)).copied().unwrap_or(NONE)
}

fn put(columns: &mut Vec<u32>, column: u16, group: u32) {
    let column = usize::from(column);
    if columns.len() <= column {
        columns.resize(column + 1, NONE);
    }
    columns[column] = group;
}

impl Trace {
    fn open(&mut self, lane: u32, child: u32, own: bool) -> u32 {
        self.groups.push(Group {
            lane,
            child,
            own,
            end: NONE,
        });
        index(self.groups.len() - 1)
    }

    fn lane_of(&self, group: u32) -> u32 {
        self.groups.get(group as usize).map_or(NONE, |g| g.lane)
    }
}

/// Finds every line's lane by its columns (the three lists of `07-graph-rendering.md` §3);
/// geometry it does not know becomes a lane of its own, never an error.
fn trace(rows: &[GraphRow]) -> Trace {
    let mut trace = Trace::default();
    let mut next_lane = 0_u32;
    let mut fresh = || {
        next_lane += 1;
        next_lane - 1
    };
    let mut above: Vec<u32> = Vec::new();
    let mut below: Vec<u32> = Vec::new();
    let mut middle: Vec<u32> = Vec::new();

    for (r, row) in rows.iter().enumerate() {
        let r = index(r);
        below.clear();
        middle.clear();
        let first = trace.segment_group.len();
        trace.segment_first.push(index(first));
        trace.segment_group.resize(first + row.segments.len(), NONE);
        let into_node = |s: &Segment| s.span == Span::Top && s.to == row.lane;

        // Main line: the primary lane; any other node: the leftmost lane meeting in it.
        let own_in = row
            .segments
            .iter()
            .filter(|s| into_node(s) && (!row.primary || s.primary))
            .min_by_key(|s| s.from)
            .map(|s| at(&above, s.from))
            .filter(|group| *group != NONE);
        let node_lane = own_in.map_or_else(&mut fresh, |group| trace.lane_of(group));
        trace.node_lane.push(node_lane);

        for (i, s) in row.segments.iter().enumerate() {
            if s.span == Span::Bottom {
                continue;
            }
            let mut group = at(&above, s.from);
            if group == NONE {
                group = trace.open(fresh(), NONE, false);
            }
            trace.segment_group[first + i] = group;
            match s.span {
                Span::Through => put(&mut below, s.to, group),
                Span::Top if into_node(s) => trace.groups[group as usize].end = r,
                _ => put(&mut middle, s.to, group),
            }
        }

        // Lanes passing the node first, so a merge into one of them finds it.
        for (i, s) in row.segments.iter().enumerate() {
            if s.span != Span::Bottom || s.arrow || s.from == row.lane {
                continue;
            }
            let mut group = at(&middle, s.from);
            if group == NONE {
                group = trace.open(fresh(), NONE, false);
            }
            trace.segment_group[first + i] = group;
            put(&mut below, s.to, group);
        }

        let first_hidden = row.segments.iter().any(|s| s.arrow && s.from == s.to);
        let mut own_done = first_hidden;
        for (i, s) in row.segments.iter().enumerate() {
            if s.span != Span::Bottom || s.from != row.lane {
                continue;
            }
            let group = if s.arrow {
                let own = s.from == s.to;
                trace.open(if own { node_lane } else { fresh() }, r, own)
            } else if !own_done {
                own_done = true;
                let group = trace.open(node_lane, r, true);
                put(&mut below, s.to, group);
                group
            } else {
                let joined = at(&below, s.to);
                if joined == NONE {
                    let group = trace.open(fresh(), r, false);
                    put(&mut below, s.to, group);
                    group
                } else {
                    trace.joins.push((joined, r));
                    joined
                }
            };
            trace.segment_group[first + i] = group;
        }
        std::mem::swap(&mut above, &mut below);
    }
    trace.segment_first.push(index(trace.segment_group.len()));
    trace
}

fn first_parent(parents: &[Vec<Option<u32>>], row: u32) -> Option<u32> {
    parents
        .get(row as usize)
        .and_then(|p| p.first().copied().flatten())
}

/// Each tip's first parents, down to the main line or a commit a tip above already took.
fn chains(rows: &[GraphRow], parents: &[Vec<Option<u32>>], tips: &[(u32, u8)]) -> Vec<u8> {
    let mut claimed = vec![0_u8; rows.len()];
    let mut order: Vec<(u32, u8)> = tips.to_vec();
    order.sort_by_key(|(row, _)| *row);
    for (tip, slot) in order {
        let mark = slot.min(PAINT_SLOT - 1) + 1;
        let mut at = Some(tip);
        while let Some(commit) = at {
            let Some(row) = rows.get(commit as usize) else {
                break;
            };
            if row.primary || claimed[commit as usize] != 0 {
                break;
            }
            claimed[commit as usize] = mark;
            at = first_parent(parents, commit);
        }
    }
    claimed
}

/// `parents`: per row, the rows of its parents as laid out, `None` when not listed.
#[must_use]
pub fn paint(rows: &[GraphRow], parents: &[Vec<Option<u32>>], spec: &PaintSpec) -> Paint {
    let trace = trace(rows);
    let claimed = chains(rows, parents, &spec.tips);
    let slot_of = |row: u32| claimed.get(row as usize).copied().unwrap_or(0);

    // First-parent lines take their child's colour, merged-in ones their parent's.
    let edge = |child: u32, own: bool, end: u32| -> u8 {
        if own {
            slot_of(child)
        } else if end == NONE {
            0
        } else {
            slot_of(end)
        }
    };
    let mut groups: Vec<u8> = trace
        .groups
        .iter()
        .map(|g| {
            if g.child == NONE {
                0
            } else {
                edge(g.child, g.own, g.end)
            }
        })
        .collect();
    for &(group, child) in &trace.joins {
        let end = trace.groups[group as usize].end;
        let slot = edge(child, false, end);
        let style = &mut groups[group as usize];
        if *style == 0 {
            *style = slot;
        }
    }

    let segment_lane = trace
        .segment_group
        .iter()
        .map(|&group| trace.lane_of(group))
        .collect();
    let segment_style = trace
        .segment_group
        .iter()
        .map(|&group| groups[group as usize])
        .collect();
    Paint {
        node_lane: trace.node_lane,
        node_style: claimed,
        segment_first: trace.segment_first,
        segment_lane,
        segment_style,
    }
}
