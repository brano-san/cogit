use crate::{Above, CommitNode, GraphRow, Lane, LayoutCursor, LongLink, NodeKind, Segment, Span};

/// The top commit's line: it keeps column 0 down its first parents (07 §3).
const TRUNK: u64 = 0;

/// One row per commit, unless the cursor looks ahead: then as [`push`].
#[must_use]
pub fn layout(commits: &[CommitNode], cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    if cursor.lookahead == 0 {
        return commits.iter().map(|node| place(node, cursor)).collect();
    }
    push(commits.to_vec(), cursor)
}

/// Streams a chunk. Knowing how long a line is needs `lookahead` commits ahead, so that
/// many stay back until the next chunk or [`finish`].
#[must_use]
pub fn push(commits: Vec<CommitNode>, cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    if cursor.lookahead == 0 {
        return commits.iter().map(|node| place(node, cursor)).collect();
    }
    let mut rows = Vec::with_capacity(commits.len());
    for node in commits {
        let row = cursor
            .next_row
            .saturating_add(u32::try_from(cursor.pending.len()).unwrap_or(u32::MAX));
        cursor.ahead.insert(node.oid.clone(), row);
        cursor.pending.push_back(node);
        while cursor.pending.len() > cursor.lookahead {
            rows.extend(place_pending(cursor));
        }
    }
    rows
}

/// The rows still held back when the history ends.
#[must_use]
pub fn finish(cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    let mut rows = Vec::with_capacity(cursor.pending.len());
    while let Some(row) = place_pending(cursor) {
        rows.push(row);
    }
    rows
}

fn place_pending(cursor: &mut LayoutCursor) -> Option<GraphRow> {
    let node = cursor.pending.pop_front()?;
    cursor.ahead.remove(&node.oid);
    Some(place(&node, cursor))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a row with 65 536 lanes is not a history anyone draws"
)]
fn column(index: usize) -> u16 {
    index as u16
}

fn waits_for(lane: &Lane, oid: &str) -> bool {
    lane.waits == oid
}

fn place(node: &CommitNode, cursor: &mut LayoutCursor) -> GraphRow {
    let row = cursor.next_row;
    cursor.next_row += 1;
    let mut above = std::mem::take(&mut cursor.above);
    let mut converging = std::mem::take(&mut cursor.converging);
    let mut leaving = std::mem::take(&mut cursor.leaving);
    let mut middle = std::mem::take(&mut cursor.middle);
    above.clear();
    converging.clear();
    leaving.clear();
    middle.clear();
    above.extend(cursor.lanes.iter().map(|lane| Above {
        id: lane.id,
        color: lane.color,
        primary: lane.primary,
    }));

    // 1. The node: on the leftmost line waiting for it, or a new one for a branch tip.
    let across = if cursor.cut_colour.is_empty() {
        None
    } else {
        cursor.cut_colour.remove(&node.oid)
    };
    let on_main = cursor.mainline.as_deref() == Some(node.oid.as_str())
        || cursor
            .lanes
            .iter()
            .any(|lane| lane.primary && waits_for(lane, &node.oid));
    let waited = cursor
        .lanes
        .iter()
        .position(|lane| waits_for(lane, &node.oid));
    let node_id = match waited {
        Some(at) => cursor.lanes[at].id,
        None => open_tip(node, cursor, across, row),
    };

    // 2. Every other line waiting for it ends in it; the lines right of it close up.
    converging.extend(
        cursor
            .lanes
            .iter()
            .filter(|lane| lane.id != node_id && waits_for(lane, &node.oid))
            .map(|lane| lane.id),
    );
    if !converging.is_empty() {
        cursor.lanes.retain(|lane| !converging.contains(&lane.id));
    }
    let node_at = cursor
        .lanes
        .iter()
        .position(|lane| lane.id == node_id)
        .unwrap_or(0);
    let color = cursor.lanes.get(node_at).map_or(0, |lane| lane.color);
    middle.extend(cursor.lanes.iter().map(|lane| lane.id));

    // 3. The first parent goes on in the node's line. Lines to one commit become one as
    // soon as both exist: the right one turns into the left one in the gap under the node.
    let shown = |parent: &String| !node.hidden.contains(parent);
    // The main line is never cut: it is the one line the eye follows all the way down.
    let cut_first = !on_main
        && node
            .parents
            .first()
            .is_some_and(|parent| shown(parent) && far(cursor, parent, row));
    let first = node
        .parents
        .first()
        .filter(|parent| shown(parent) && !cut_first)
        .cloned();
    // The node's own line below it, and the line its first parent goes on in.
    let mut own_at = None;
    let mut first_into = None;
    let mut merged = None;
    match first {
        None => {
            cursor.lanes.remove(node_at);
        }
        Some(parent) => {
            let other = cursor
                .lanes
                .iter()
                .position(|lane| lane.id != node_id && waits_for(lane, &parent));
            match other {
                Some(left) if left < node_at => {
                    cursor.lanes[left].primary |= on_main;
                    first_into = Some(cursor.lanes[left].id);
                    cursor.lanes.remove(node_at);
                }
                other => {
                    let ends = ends_of(cursor, &parent);
                    let mut primary = on_main;
                    if let Some(right) = other {
                        primary |= cursor.lanes[right].primary;
                        merged = Some(cursor.lanes.remove(right).id);
                    }
                    let lane = &mut cursor.lanes[node_at];
                    lane.waits = parent;
                    lane.ends = ends;
                    lane.primary = primary;
                    own_at = Some(node_at);
                    first_into = Some(node_id);
                }
            }
        }
    }
    leaving.extend(first_into);

    // The other parents join a line already waiting for them, else get a new one right of
    // the node, past the lines that end before theirs.
    let mut insert_at = own_at.map_or(node_at, |at| at + 1);
    let mut cut_later = Vec::new();
    for parent in node.parents.iter().skip(1).filter(|parent| shown(parent)) {
        if let Some(lane) = cursor.lanes.iter().find(|lane| waits_for(lane, parent)) {
            leaving.push(lane.id);
            continue;
        }
        // A line already on its way costs nothing more; only a new one is worth cutting.
        if far(cursor, parent, row) {
            cut_later.push(parent);
            continue;
        }
        let lane = Lane {
            id: cursor.take_id(),
            waits: parent.clone(),
            color: cursor.take_color(),
            primary: false,
            ends: ends_of(cursor, parent),
        };
        leaving.push(lane.id);
        insert_at = settle(cursor, insert_at, lane) + 1;
    }

    // 4. Each line's column at the top edge, at the node and at the bottom edge: a line
    // passing the row moves between them in the gaps, a turn spanning the gap (07 §5).
    let (mut mid_from, mut to_from) = (0, 0);
    let mut segments = Vec::with_capacity(above.len() + leaving.len());
    for (index, lane) in above.iter().enumerate() {
        let line = |from: usize, to: usize, span: Span| Segment {
            from: column(from),
            to: column(to),
            span,
            primary: lane.primary,
            color: lane.color,
            arrow: false,
        };
        if lane.id == node_id || converging.contains(&lane.id) {
            segments.push(line(index, node_at, Span::Top));
            continue;
        }
        let Some(mid) = find_from(&middle, mid_from, |id| *id == lane.id) else {
            continue;
        };
        mid_from = mid + 1;
        let to = if merged == Some(lane.id) {
            node_at
        } else {
            let Some(to) = find_from(&cursor.lanes, to_from, |other| other.id == lane.id) else {
                continue;
            };
            to_from = to + 1;
            to
        };
        if index == mid && mid == to {
            segments.push(line(index, to, Span::Through));
        } else {
            segments.push(line(index, mid, Span::Top));
            segments.push(line(mid, to, Span::Bottom));
        }
    }
    for &id in &leaving {
        if let Some(to) = cursor.lanes.iter().position(|lane| lane.id == id) {
            segments.push(Segment {
                from: column(node_at),
                to: column(to),
                span: Span::Bottom,
                // A side commit joining the main line is still a side line.
                primary: on_main && first_into == Some(id),
                color: cursor.lanes[to].color,
                arrow: false,
            });
        }
    }

    // Straight down for the first parent; a later one's stub leans right, off that line.
    let first_hidden = node.parents.first().is_some_and(|parent| !shown(parent));
    let later_hidden = node.parents.iter().skip(1).any(|parent| !shown(parent));
    for (lean, hidden) in [(0, first_hidden), (1, later_hidden)] {
        if !hidden {
            continue;
        }
        segments.push(Segment {
            from: column(node_at),
            to: column(node_at + lean),
            span: Span::Bottom,
            primary: on_main && lean == 0,
            color,
            arrow: true,
        });
    }

    // A cut link: a stub under the node towards the parent, one above it from the children.
    let mut links = Vec::new();
    let into = cursor
        .cut_into
        .remove(&node.oid)
        .map(|mut children| {
            children.reverse();
            children
        })
        .unwrap_or_default();
    let first_cut: Vec<String> = node
        .parents
        .first()
        .filter(|_| cut_first)
        .cloned()
        .into_iter()
        .collect();
    let later_cut: Vec<String> = cut_later.iter().map(|parent| (*parent).clone()).collect();
    for (lean, span, ends) in [
        (0, Span::Bottom, first_cut),
        (1, Span::Bottom, later_cut),
        (1, Span::Top, into),
    ] {
        if ends.is_empty() {
            continue;
        }
        let segment = column(segments.len());
        segments.push(Segment {
            from: column(node_at),
            to: column(node_at + lean),
            span,
            primary: false,
            color,
            arrow: true,
        });
        for oid in ends {
            if span == Span::Bottom {
                cursor
                    .cut_into
                    .entry(oid.clone())
                    .or_default()
                    .push(node.oid.clone());
                if lean == 0 {
                    cursor.cut_colour.insert(oid.clone(), color);
                }
            }
            links.push(LongLink { segment, oid });
        }
    }

    let width = segments
        .iter()
        .flat_map(|segment| {
            [
                segment.from,
                if segment.arrow {
                    segment.from
                } else {
                    segment.to
                },
            ]
        })
        .chain([column(node_at)])
        .max()
        .unwrap_or(0)
        + 1;
    cursor.above = above;
    cursor.converging = converging;
    cursor.leaving = leaving;
    cursor.middle = middle;

    GraphRow {
        row,
        lane: column(node_at),
        color,
        kind: match node.parents.len() {
            0 => NodeKind::Root,
            1 => NodeKind::Normal,
            _ => NodeKind::Merge,
        },
        primary: on_main,
        width,
        segments,
        links,
    }
}

/// The index of the first match at or after `from`.
fn find_from<T>(items: &[T], from: usize, is: impl Fn(&T) -> bool) -> Option<usize> {
    items.get(from..)?.iter().position(is).map(|at| at + from)
}

/// The row of a commit within the lookahead, else `u32::MAX`.
fn ends_of(cursor: &LayoutCursor, oid: &str) -> u32 {
    cursor.ahead.get(oid).copied().unwrap_or(u32::MAX)
}

fn far(cursor: &LayoutCursor, parent: &str, row: u32) -> bool {
    cursor.long_links > 0
        && cursor.ahead.get(parent).is_none_or(|&at| {
            usize::try_from(at.saturating_sub(row)).unwrap_or(usize::MAX) > cursor.long_links
        })
}

/// A commit nobody was waiting for. Its first parent already awaited: right beside that
/// line, the lines beyond stepping aside for the row, and its own line turns into that one
/// under it. Otherwise past the lines that end before its line does. Below a cut
/// first-parent link it takes the colour of the child above the cut, `across`.
fn open_tip(node: &CommitNode, cursor: &mut LayoutCursor, across: Option<u8>, row: u32) -> u64 {
    let parent = node
        .parents
        .first()
        .filter(|parent| !node.hidden.contains(parent));
    let beside =
        parent.and_then(|parent| cursor.lanes.iter().position(|lane| waits_for(lane, parent)));
    // Taken either way, so the lanes after it keep their colours.
    let next = cursor.take_color();
    let lane = Lane {
        id: cursor.take_id(),
        waits: node.oid.clone(),
        color: across.unwrap_or(next),
        primary: false,
        ends: parent.map_or(row, |parent| ends_of(cursor, parent)),
    };
    let id = lane.id;
    match beside {
        Some(at) => cursor.lanes.insert(at + 1, lane),
        None => {
            let from = usize::from(cursor.lanes.first().is_some_and(|lane| lane.id == TRUNK));
            settle(cursor, from, lane);
        }
    }
    id
}

/// Puts a new line at `from` or right of it, past the lines that end no later than it;
/// those that go on longer step aside. Returns the column.
fn settle(cursor: &mut LayoutCursor, from: usize, lane: Lane) -> usize {
    let at = find_from(&cursor.lanes, from, |other| other.ends > lane.ends)
        .unwrap_or(cursor.lanes.len());
    cursor.lanes.insert(at, lane);
    at
}

/// A commit a filter leaves out of a graph drawn with lines (F-561): every line waiting for
/// it goes on to its first parent, so a match joins the nearest match down that path. No
/// parent keeps the line waiting: the history goes on below the list.
pub fn pass_through(cursor: &mut LayoutCursor, oid: &str, first_parent: Option<&str>) {
    let Some(parent) = first_parent else {
        return;
    };
    if cursor.mainline.as_deref() == Some(oid) {
        cursor.mainline = Some(parent.to_owned());
    }
    for lane in &mut cursor.lanes {
        if waits_for(lane, oid) {
            parent.clone_into(&mut lane.waits);
        }
    }
}
