use crate::{Above, CommitNode, GraphRow, Lane, LayoutCursor, LongLink, NodeKind, Segment, Span};

/// One row per commit, unless the cursor cuts long links: then as [`push`].
#[must_use]
pub fn layout(commits: &[CommitNode], cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    if cursor.long_links == 0 {
        return commits.iter().map(|node| place(node, cursor)).collect();
    }
    push(commits.to_vec(), cursor)
}

/// Streams a chunk. Cutting long links needs `long_links` commits of lookahead, so that
/// many stay back until the next chunk or [`finish`].
#[must_use]
pub fn push(commits: Vec<CommitNode>, cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    if cursor.long_links == 0 {
        return commits.iter().map(|node| place(node, cursor)).collect();
    }
    let mut rows = Vec::with_capacity(commits.len());
    for node in commits {
        cursor.ahead.insert(node.oid.clone());
        cursor.pending.push_back(node);
        while cursor.pending.len() > cursor.long_links {
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
    lane.waits.as_deref() == Some(oid)
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
        drawn: lane.drawn,
        primary: lane.primary,
    }));
    cursor.lanes.retain(|lane| !lane.ended);

    // 1. The node: the leftmost lane waiting for it, or a new one for a branch tip.
    let across = if cursor.cut_colour.is_empty() {
        None
    } else {
        cursor.cut_colour.remove(&node.oid)
    };
    let on_main = cursor.reserved
        && cursor
            .lanes
            .first()
            .is_some_and(|lane| waits_for(lane, &node.oid));
    let node_id = match cursor
        .lanes
        .iter()
        .position(|lane| waits_for(lane, &node.oid))
    {
        Some(at) => cursor.lanes[at].id,
        None => open_tip(node, cursor, across),
    };
    let node_at = cursor
        .lanes
        .iter()
        .position(|lane| lane.id == node_id)
        .unwrap_or(0);

    // 2. Every other lane waiting for it ends in it.
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
    middle.extend(cursor.lanes.iter().map(|lane| lane.id));

    // 3. The first parent continues the lane; the others join a lane already waiting for
    // them or open one right of the node, in parent order.
    let shown = |parent: &String| !node.hidden.contains(parent);
    // Column 0 is never cut: it is the one line the eye follows all the way down.
    let cut_first = !on_main
        && node
            .parents
            .first()
            .is_some_and(|parent| shown(parent) && far(cursor, parent));
    let first = node
        .parents
        .first()
        .filter(|parent| shown(parent) && !cut_first)
        .cloned();
    let node_index = cursor
        .lanes
        .iter()
        .position(|lane| lane.id == node_id)
        .unwrap_or(0);
    let continues = first.is_some();
    {
        let lane = &mut cursor.lanes[node_index];
        lane.waits = first;
        lane.drawn = continues;
        lane.ended = !continues && !lane.primary;
    }
    if continues {
        leaving.push(node_id);
    }
    let mut insert_at = node_index + 1;
    let mut cut_later = Vec::new();
    for parent in node.parents.iter().skip(1).filter(|parent| shown(parent)) {
        if let Some(lane) = cursor.lanes.iter().find(|lane| waits_for(lane, parent)) {
            if lane.id != node_id {
                leaving.push(lane.id);
            }
            continue;
        }
        // A lane already on its way costs nothing more; only a new one is worth cutting.
        if far(cursor, parent) {
            cut_later.push(parent);
            continue;
        }
        let lane = Lane {
            id: cursor.take_id(),
            waits: Some(parent.clone()),
            color: cursor.take_color(),
            drawn: true,
            primary: false,
            ended: false,
        };
        leaving.push(lane.id);
        cursor.lanes.insert(insert_at, lane);
        insert_at += 1;
    }

    // 4. What moved is what gets drawn. A lane passing the node makes room for what
    // arrives in the upper half and for what leaves in the lower one, beside the node's own
    // lines and never in its column at the ring.
    let below = |id: u64, lanes: &[Lane]| lanes.iter().position(|lane| lane.id == id);
    let mut segments = Vec::with_capacity(above.len() + leaving.len());
    for (index, lane) in above.iter().enumerate() {
        if !lane.drawn {
            continue;
        }
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
        let (Some(mid), Some(to)) = (
            middle.iter().position(|id| *id == lane.id),
            below(lane.id, &cursor.lanes),
        ) else {
            continue;
        };
        if index == mid && mid == to {
            segments.push(line(index, to, Span::Through));
        } else {
            segments.push(line(index, mid, Span::Top));
            segments.push(line(mid, to, Span::Bottom));
        }
    }
    for &id in &leaving {
        if let Some(to) = below(id, &cursor.lanes) {
            let lane = &cursor.lanes[to];
            segments.push(Segment {
                from: column(node_at),
                to: column(to),
                span: Span::Bottom,
                // A side commit joining the main line is still a side line.
                primary: on_main && id == node_id,
                color: lane.color,
                arrow: false,
            });
        }
    }

    let color = above.iter().find(|lane| lane.id == node_id).map_or_else(
        || cursor.lanes.get(node_index).map_or(0, |lane| lane.color),
        |lane| lane.color,
    );
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

fn far(cursor: &LayoutCursor, parent: &str) -> bool {
    cursor.long_links > 0 && !cursor.ahead.contains(parent)
}

/// A commit nobody was waiting for starts a lane of its own, beside the lane its first
/// parent is already on when there is one, else at the right; never in column 0. Below a
/// cut first-parent link it takes the colour of the child above the cut, `across`.
fn open_tip(node: &CommitNode, cursor: &mut LayoutCursor, across: Option<u8>) -> u64 {
    let beside = node
        .parents
        .first()
        .filter(|parent| !node.hidden.contains(parent))
        .and_then(|parent| cursor.lanes.iter().position(|lane| waits_for(lane, parent)));
    let floor = usize::from(cursor.reserved);
    let at = beside
        .map_or(cursor.lanes.len(), |lane| lane + 1)
        .max(floor);
    // Taken either way, so the lanes after it keep their colours.
    let next = cursor.take_color();
    let lane = Lane {
        id: cursor.take_id(),
        waits: Some(node.oid.clone()),
        color: across.unwrap_or(next),
        drawn: true,
        primary: false,
        ended: false,
    };
    let id = lane.id;
    cursor.lanes.insert(at.min(cursor.lanes.len()), lane);
    id
}
