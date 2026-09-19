//! Lane allocation: turning a commit list into columns and curves.
//!
//! The algorithm is the classic "active lanes" sweep used by `git log --graph` and by
//! every graphical client. It is specified in `doc/07-graph-rendering.md` section 3.
//!
//! Edges are emitted **per row band** — each one joins row `r-1` to row `r` — rather
//! than spanning from a commit to a distant parent. That is what makes the layout
//! streamable: a chunk can be drawn without knowing where its parents will land.

use crate::{CommitNode, EdgeKind, GraphEdge, GraphLayout, LaneAssignment, LayoutCursor, NodeKind};

/// Places a chunk of commits into lanes, continuing from `cursor`.
///
/// `commits` must already be in the order they will be drawn, newest first. The cursor
/// carries lane occupancy, colours and the row counter across chunks, which is what
/// keeps a branch the same colour while the user scrolls.
#[must_use]
pub fn layout(commits: &[CommitNode], cursor: &mut LayoutCursor) -> GraphLayout {
    let mut out = GraphLayout::default();

    for node in commits {
        let row = cursor.next_row;
        let commit_lane = choose_lane(node, cursor);

        emit_band(node, commit_lane, row, cursor, &mut out.edges);

        // Every line that was waiting for this commit ends here.
        for lane in 0..cursor.active.len() {
            if cursor.active[lane].as_deref() == Some(node.oid.as_str()) {
                cursor.active[lane] = None;
                cursor.origins[lane].clear();
            } else if cursor.active[lane].is_some() {
                // Lines that merely pass through continue straight down.
                cursor.origins[lane].clear();
                cursor.origins[lane].push(lane_index(lane));
            }
        }

        place_parents(node, commit_lane, cursor);

        out.lanes.push(LaneAssignment {
            row,
            lane: lane_index(commit_lane),
            color: cursor.lane_colors[commit_lane],
            kind: node_kind(node),
        });
        out.max_lane = out.max_lane.max(lane_index(commit_lane));
        out.max_lane = out.max_lane.max(widest(cursor));

        trim_trailing_free_lanes(cursor);
        cursor.next_row += 1;
    }

    out
}

fn node_kind(node: &CommitNode) -> NodeKind {
    match node.parents.len() {
        0 => NodeKind::Root,
        1 => NodeKind::Normal,
        _ => NodeKind::Merge,
    }
}

/// Lane indices are small by construction; a repository needing more than 65 535
/// simultaneous branches is not a case worth carrying a wider type for.
#[expect(
    clippy::cast_possible_truncation,
    reason = "lane count is bounded in practice"
)]
fn lane_index(lane: usize) -> u16 {
    lane as u16
}

fn widest(cursor: &LayoutCursor) -> u16 {
    lane_index(cursor.active.len().saturating_sub(1))
}

/// Picks the column for a commit, creating one if nothing was expecting it.
fn choose_lane(node: &CommitNode, cursor: &mut LayoutCursor) -> usize {
    // Reuse the leftmost lane already waiting for this commit, so merges close towards
    // the mainline instead of drifting right.
    if let Some(lane) = cursor
        .active
        .iter()
        .position(|slot| slot.as_deref() == Some(node.oid.as_str()))
    {
        return lane;
    }

    let lane = cursor.first_free_lane();
    grow_to(cursor, lane + 1);
    cursor.lane_colors[lane] = cursor.take_color();
    cursor.origins[lane].clear();
    lane
}

/// Emits the curves joining the previous row to this one.
fn emit_band(
    node: &CommitNode,
    commit_lane: usize,
    row: u32,
    cursor: &LayoutCursor,
    edges: &mut Vec<GraphEdge>,
) {
    if row == 0 {
        return;
    }
    for (lane, slot) in cursor.active.iter().enumerate() {
        let Some(waiting_for) = slot else { continue };
        let arrives_here = waiting_for == &node.oid;
        let to_lane = if arrives_here { commit_lane } else { lane };
        let kind = if !arrives_here {
            EdgeKind::Crossing
        } else if lane == commit_lane {
            EdgeKind::Direct
        } else {
            EdgeKind::Merge
        };

        // A lane can be fed by more than one line when two children share a parent.
        for &from_lane in &cursor.origins[lane] {
            edges.push(GraphEdge {
                from_row: row - 1,
                from_lane,
                to_row: row,
                to_lane: lane_index(to_lane),
                color: cursor.lane_colors[lane],
                kind,
            });
        }
    }
}

/// Reserves a lane for each parent, so the sweep knows where to expect them.
fn place_parents(node: &CommitNode, commit_lane: usize, cursor: &mut LayoutCursor) {
    for (index, parent) in node.parents.iter().enumerate() {
        if let Some(existing) = cursor
            .active
            .iter()
            .position(|slot| slot.as_deref() == Some(parent.as_str()))
        {
            // Another child already reserved this parent; both lines converge on it.
            cursor.origins[existing].push(lane_index(commit_lane));
            continue;
        }

        // The first parent inherits the commit's own lane, which is what keeps the
        // mainline vertical rather than zig-zagging (doc/07-graph-rendering.md).
        let lane = if index == 0 {
            commit_lane
        } else {
            let free = cursor.first_free_lane();
            grow_to(cursor, free + 1);
            cursor.lane_colors[free] = cursor.take_color();
            free
        };

        cursor.active[lane] = Some(parent.clone());
        cursor.origins[lane].clear();
        cursor.origins[lane].push(lane_index(commit_lane));
    }
}

fn grow_to(cursor: &mut LayoutCursor, len: usize) {
    if cursor.active.len() < len {
        cursor.active.resize(len, None);
        cursor.lane_colors.resize(len, 0);
        cursor.origins.resize(len, Vec::new());
    }
}

/// Drops empty lanes on the right so the graph does not creep sideways for ever.
fn trim_trailing_free_lanes(cursor: &mut LayoutCursor) {
    while cursor.active.last().is_some_and(Option::is_none) {
        cursor.active.pop();
        cursor.lane_colors.pop();
        cursor.origins.pop();
    }
}
