use crate::{Above, CommitNode, GraphRow, Lane, LayoutCursor, NodeKind, Segment, Span};

#[must_use]
pub fn layout(commits: &[CommitNode], cursor: &mut LayoutCursor) -> Vec<GraphRow> {
    commits.iter().map(|node| place(node, cursor)).collect()
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
    above.clear();
    converging.clear();
    leaving.clear();
    above.extend(cursor.lanes.iter().map(|lane| Above {
        id: lane.id,
        color: lane.color,
        drawn: lane.drawn,
        primary: lane.primary,
    }));

    // 1. The node: the leftmost lane waiting for it, or a new one for a branch tip.
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
        None => open_tip(node, cursor),
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

    // 3. The first parent continues the lane; the others join a lane already waiting for
    // them or open one right of the node, in parent order.
    let shown = |parent: &String| !node.hidden.contains(parent);
    let first = node.parents.first().filter(|parent| shown(parent)).cloned();
    let node_index = cursor
        .lanes
        .iter()
        .position(|lane| lane.id == node_id)
        .unwrap_or(0);
    let continues = first.is_some();
    {
        let lane = &mut cursor.lanes[node_index];
        lane.waits = first;
        lane.drawn = continues || !lane.primary;
    }
    if continues {
        leaving.push(node_id);
    }
    let mut insert_at = node_index + 1;
    for parent in node.parents.iter().skip(1).filter(|parent| shown(parent)) {
        if let Some(lane) = cursor.lanes.iter().find(|lane| waits_for(lane, parent)) {
            if lane.id != node_id {
                leaving.push(lane.id);
            }
            continue;
        }
        let lane = Lane {
            id: cursor.take_id(),
            waits: Some(parent.clone()),
            color: cursor.take_color(),
            drawn: true,
            primary: false,
        };
        leaving.push(lane.id);
        cursor.lanes.insert(insert_at, lane);
        insert_at += 1;
    }

    // 4. Close the gaps. Column 0 stays, waiting or empty, for as long as it is reserved.
    let reserved = cursor.reserved;
    cursor
        .lanes
        .retain(|lane| lane.waits.is_some() || (reserved && lane.primary));

    // 5. What moved from the top edge to the bottom edge is what gets drawn.
    let below = |id: u64, lanes: &[Lane]| lanes.iter().position(|lane| lane.id == id);
    let mut segments = Vec::with_capacity(above.len() + leaving.len());
    for (index, lane) in above.iter().enumerate() {
        if !lane.drawn {
            continue;
        }
        let segment = if lane.id == node_id || converging.contains(&lane.id) {
            Some((index, node_at, Span::Top))
        } else {
            below(lane.id, &cursor.lanes).map(|to| (index, to, Span::Through))
        };
        if let Some((from, to, span)) = segment {
            segments.push(Segment {
                from: column(from),
                to: column(to),
                span,
                primary: lane.primary,
                color: lane.color,
                arrow: false,
            });
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
    if node.parents.iter().any(|parent| !shown(parent)) {
        segments.push(Segment {
            from: column(node_at),
            to: column(node_at),
            span: Span::Bottom,
            primary: on_main,
            color,
            arrow: true,
        });
    }

    let width = segments
        .iter()
        .flat_map(|segment| [segment.from, segment.to])
        .chain([column(node_at)])
        .max()
        .unwrap_or(0)
        + 1;
    cursor.above = above;
    cursor.converging = converging;
    cursor.leaving = leaving;

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
    }
}

/// A commit nobody was waiting for starts a lane of its own, beside the lane its first
/// parent is already on when there is one, else at the right; never in column 0.
fn open_tip(node: &CommitNode, cursor: &mut LayoutCursor) -> u64 {
    let beside = node
        .parents
        .first()
        .filter(|parent| !node.hidden.contains(parent))
        .and_then(|parent| cursor.lanes.iter().position(|lane| waits_for(lane, parent)));
    let floor = usize::from(cursor.reserved);
    let at = beside
        .map_or(cursor.lanes.len(), |lane| lane + 1)
        .max(floor);
    let lane = Lane {
        id: cursor.take_id(),
        waits: Some(node.oid.clone()),
        color: cursor.take_color(),
        drawn: true,
        primary: false,
    };
    let id = lane.id;
    cursor.lanes.insert(at.min(cursor.lanes.len()), lane);
    id
}
