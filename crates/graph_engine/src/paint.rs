//! Colour over a finished layout; the layout itself never changes for it (R-340).

use std::collections::HashMap;

use crate::{GraphRow, Segment, Span};

pub const PAINT_SLOT: u8 = 0x0f;
pub const PAINT_DIM: u8 = 0x10;

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaintSpec {
    /// Tips by row with their palette slot; a tip higher up claims its line first.
    pub tips: Vec<(u32, u8)>,
    /// All but this commit, its ancestors and descendants is dimmed.
    pub ancestry_of: Option<u32>,
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
    joins: Vec<(u32, u32, bool)>,
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
fn trace(
    rows: &[GraphRow],
    parents: &[Vec<Option<u32>>],
    row_of: &dyn Fn(&str) -> Option<u32>,
) -> Trace {
    let mut trace = Trace::default();
    // Lane of the stub under a cut link's child, by (child, parent), for the stub above.
    let mut stubs: HashMap<(u32, u32), u32> = HashMap::new();
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
        let node_lane = match own_in {
            Some(group) => trace.lane_of(group),
            None if !row.primary => {
                across_cut(row, r, parents, row_of, &stubs).unwrap_or_else(&mut fresh)
            }
            None => fresh(),
        };
        trace.node_lane.push(node_lane);

        let ends = |i: usize| {
            row.links
                .iter()
                .filter(move |link| usize::from(link.segment) == i)
                .filter_map(|link| row_of(&link.oid))
        };
        for (i, s) in row.segments.iter().enumerate() {
            if s.span == Span::Bottom {
                continue;
            }
            if s.arrow {
                let children: Vec<u32> = ends(i).collect();
                let own = |c: u32| first_parent(parents, c) == Some(r);
                let Some(&lead) = children.iter().find(|c| own(**c)).or(children.first()) else {
                    trace.segment_group[first + i] = trace.open(fresh(), NONE, false);
                    continue;
                };
                let lane = stubs.get(&(lead, r)).copied().unwrap_or_else(&mut fresh);
                let group = trace.open(lane, lead, own(lead));
                trace.groups[group as usize].end = r;
                for &child in children.iter().filter(|c| **c != lead) {
                    trace.joins.push((group, child, own(child)));
                }
                trace.segment_group[first + i] = group;
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
                let lane = if own { node_lane } else { fresh() };
                let group = trace.open(lane, r, own);
                for parent in ends(i) {
                    stubs.insert((r, parent), lane);
                    let end = &mut trace.groups[group as usize].end;
                    if *end == NONE {
                        *end = parent;
                    }
                }
                group
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
                    trace.joins.push((joined, r, false));
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

/// The lane of the stub above `row` from a child whose first-parent link to it was cut:
/// the branch goes on across the cut as if the line were whole (07 §5). The child is the
/// one that stub is traced from.
fn across_cut(
    row: &GraphRow,
    r: u32,
    parents: &[Vec<Option<u32>>],
    row_of: &dyn Fn(&str) -> Option<u32>,
    stubs: &HashMap<(u32, u32), u32>,
) -> Option<u32> {
    row.segments
        .iter()
        .enumerate()
        .filter(|(_, s)| s.arrow && s.span == Span::Top)
        .find_map(|(i, _)| {
            row.links
                .iter()
                .filter(|link| usize::from(link.segment) == i)
                .filter_map(|link| row_of(&link.oid))
                .find(|child| first_parent(parents, *child) == Some(r))
                .and_then(|child| stubs.get(&(child, r)).copied())
        })
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

const DESCENDANT: u8 = 1;
const CHOSEN: u8 = 2;
const ANCESTOR: u8 = 3;

/// Rows sit below every child: one pass up finds the descendants, one pass down the ancestors.
fn ancestry(parents: &[Vec<Option<u32>>], chosen: u32, len: usize) -> Vec<u8> {
    let mut kin = vec![0_u8; len];
    let chosen = chosen as usize;
    if chosen >= len {
        return kin;
    }
    kin[chosen] = CHOSEN;
    for row in (0..chosen).rev() {
        let leads_down = parents.get(row).is_some_and(|p| {
            p.iter().flatten().any(|&parent| {
                kin.get(parent as usize)
                    .is_some_and(|k| *k == CHOSEN || *k == DESCENDANT)
            })
        });
        if leads_down {
            kin[row] = DESCENDANT;
        }
    }
    for row in chosen..len {
        if kin[row] < CHOSEN {
            continue;
        }
        for &parent in parents.get(row).into_iter().flatten().flatten() {
            if let Some(k) = kin.get_mut(parent as usize) {
                *k = ANCESTOR;
            }
        }
    }
    kin
}

/// `parents`: per row, the rows of its parents as laid out, `None` when not listed;
/// `row_of` finds the row of a commit at the far end of a cut link.
#[must_use]
pub fn paint(
    rows: &[GraphRow],
    parents: &[Vec<Option<u32>>],
    row_of: &dyn Fn(&str) -> Option<u32>,
    spec: &PaintSpec,
) -> Paint {
    let trace = trace(rows, parents, row_of);
    let claimed = chains(rows, parents, &spec.tips);
    let kin = spec
        .ancestry_of
        .map(|chosen| ancestry(parents, chosen, rows.len()));
    let slot_of = |row: u32| claimed.get(row as usize).copied().unwrap_or(0);
    let kin_of = |row: u32| {
        kin.as_ref()
            .and_then(|k| k.get(row as usize).copied())
            .unwrap_or(0)
    };

    // First-parent lines take their child's colour, merged-in ones their parent's; a line
    // is lit when both its ends are kin of the chosen commit.
    let edge = |child: u32, own: bool, end: u32| -> (u8, bool) {
        let parent = if end != NONE {
            Some(end)
        } else if own {
            first_parent(parents, child)
        } else {
            None
        };
        let slot = if own {
            slot_of(child)
        } else {
            parent.map_or(0, slot_of)
        };
        let lit = kin_of(child) != 0 && parent.map_or(kin_of(child) >= CHOSEN, |p| kin_of(p) != 0);
        (slot, lit)
    };
    let mut groups: Vec<(u8, bool)> = trace
        .groups
        .iter()
        .map(|g| {
            if g.child == NONE {
                (0, g.end != NONE && kin_of(g.end) != 0)
            } else {
                edge(g.child, g.own, g.end)
            }
        })
        .collect();
    for &(group, child, own) in &trace.joins {
        let end = trace.groups[group as usize].end;
        let (slot, lit) = edge(child, own, end);
        let style = &mut groups[group as usize];
        if style.0 == 0 {
            style.0 = slot;
        }
        style.1 |= lit;
    }

    let dim = |lit: bool| if kin.is_some() && !lit { PAINT_DIM } else { 0 };
    let node_style = (0..rows.len())
        .map(|row| {
            let row = index(row);
            slot_of(row) | dim(kin_of(row) != 0)
        })
        .collect();
    let segment_lane = trace
        .segment_group
        .iter()
        .map(|&group| trace.lane_of(group))
        .collect();
    let segment_style = trace
        .segment_group
        .iter()
        .map(|&group| {
            let (slot, lit) = groups[group as usize];
            slot | dim(lit)
        })
        .collect();
    Paint {
        node_lane: trace.node_lane,
        node_style,
        segment_first: trace.segment_first,
        segment_lane,
        segment_style,
    }
}
