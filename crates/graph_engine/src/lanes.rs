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
    middle.extend(cursor.lanes.iter().map(|lane| lane.id));

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
        lane.drawn = continues;
        lane.ended = !continues && !lane.primary;
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
        ended: false,
    };
    let id = lane.id;
    cursor.lanes.insert(at.min(cursor.lanes.len()), lane);
    id
}
