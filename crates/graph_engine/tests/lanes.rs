// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The lane layout: a list of columns, each holding a lane or free. A lane keeps its column
//! from its first row to its last: a column given back stays empty until a new lane takes it,
//! or a lane moves into it at its own node; a line never turns between nodes (R-765). Lane 0 is the
//! top commit's first parents; HEAD reserves nothing.

use graph_engine::{
    CommitNode, GraphRow, LayoutCursor, NodeKind, Span, finish, layout, mainline_tip,
};

fn commits(spec: &[(&str, &[&str])]) -> Vec<CommitNode> {
    spec.iter()
        .map(|(oid, parents)| CommitNode {
            oid: (*oid).to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            hidden: Vec::new(),
        })
        .collect()
}

fn run(spec: &[(&str, &[&str])], mainline: Option<&str>) -> Vec<GraphRow> {
    let mut cursor = LayoutCursor::with_mainline(mainline.map(str::to_owned));
    layout(&commits(spec), &mut cursor)
}

fn segs(row: &GraphRow, span: Span) -> Vec<(u16, u16)> {
    let mut found: Vec<(u16, u16)> = row
        .segments
        .iter()
        .filter(|seg| seg.span == span)
        .map(|seg| (seg.from, seg.to))
        .collect();
    found.sort_unstable();
    found
}

// --- the seven cases of the specification --------------------------------------------

#[test]
fn a_linear_history_is_one_lane_of_vertical_segments() {
    let rows = run(&[("c", &["b"]), ("b", &["a"]), ("a", &[])], Some("c"));

    assert!(rows.iter().all(|row| row.lane == 0 && row.width == 1));
    assert!(
        rows.iter()
            .flat_map(|row| &row.segments)
            .all(|seg| seg.from == 0 && seg.to == 0),
        "{rows:?}"
    );
}

#[test]
fn a_branch_merged_back_joins_the_main_line_as_soon_as_both_wait_for_its_fork() {
    let rows = run(
        &[
            ("m4", &["m3"]),
            ("m3", &["m2", "t1"]),
            ("t1", &["m1"]),
            ("m2", &["m1"]),
            ("m1", &["m0"]),
            ("m0", &[]),
        ],
        Some("m4"),
    );

    let widths: Vec<u16> = rows.iter().map(|row| row.width).collect();
    assert_eq!(widths, vec![1, 2, 2, 2, 1, 1]);
    assert_eq!(
        segs(&rows[1], Span::Bottom),
        vec![(0, 0), (0, 1)],
        "t1 leaves the merge"
    );
    assert_eq!(rows[2].lane, 1, "the branch sits right of the main line");
    assert_eq!(
        segs(&rows[3], Span::Bottom),
        vec![(0, 0), (1, 0)],
        "and turns into the main line under m2, which waits for m1 too"
    );
    for row in &rows {
        for seg in row.segments.iter().filter(|seg| seg.primary) {
            assert_eq!(
                (seg.from, seg.to),
                (0, 0),
                "row {}: the main line moved",
                row.row
            );
        }
    }
}

#[test]
fn a_lane_ending_moves_the_lines_right_of_it_left_in_the_gap() {
    let rows = run(
        &[
            ("m3", &["m2", "a1", "b2"]),
            ("a1", &["m1"]),
            ("b2", &["b1"]),
            ("m2", &["m1"]),
            ("m1", &["m0"]),
            ("b1", &["m0"]),
            ("m0", &[]),
        ],
        Some("m3"),
    );

    let m2 = &rows[3];
    assert_eq!(
        segs(m2, Span::Bottom),
        vec![(0, 0), (1, 0), (2, 1)],
        "a's line joins the main line under m2, b's moves into its column"
    );
    assert_eq!(rows[4].width, 2);
    assert_eq!(rows[5].lane, 1);
}

#[test]
fn an_octopus_opens_its_new_lanes_right_of_the_node_in_parent_order() {
    let rows = run(
        &[
            ("top", &["m"]),
            ("t", &["z"]),
            ("m", &["a", "b", "c"]),
            ("a", &["r"]),
            ("b", &["r"]),
            ("c", &["r"]),
            ("z", &["r"]),
            ("r", &[]),
        ],
        Some("m"),
    );

    let m = &rows[2];
    assert_eq!(m.lane, 0);
    assert_eq!(
        segs(m, Span::Bottom),
        vec![(0, 0), (0, 2), (0, 3)],
        "the lane already there stays; the new ones take the next free columns: {:?}",
        m.segments
    );
    assert_eq!(segs(m, Span::Through), vec![(1, 1)]);
    assert_eq!(rows[4].lane, 2, "b takes the first new column");
    assert_eq!(
        rows[5].lane, 2,
        "c the second, moved left once b's line is gone"
    );
}

/// On any history column 0 is the top commit's first-parent chain, unbroken, and nothing
/// else; here the primary ref is that commit too.
#[test]
fn the_main_line_holds_column_zero_on_random_histories() {
    let mut seed = 0x2545_f491_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };

    for round in 0..300 {
        let count = 3 + next(40);
        let names: Vec<String> = (0..count).map(|i| format!("c{i}")).collect();
        let mut spec: Vec<(String, Vec<String>)> = Vec::new();
        for i in 0..count {
            let older = count - i - 1;
            let mut parents = Vec::new();
            if older > 0 {
                parents.push(names[i + 1 + next(older.min(4))].clone());
                if older > 1 && next(3) == 0 {
                    let other = names[i + 1 + next(older)].clone();
                    if !parents.contains(&other) {
                        parents.push(other);
                    }
                }
            }
            spec.push((names[i].clone(), parents));
        }

        let nodes: Vec<CommitNode> = spec
            .iter()
            .map(|(oid, parents)| CommitNode {
                oid: oid.clone(),
                parents: parents.clone(),
                hidden: Vec::new(),
            })
            .collect();
        let mut cursor =
            LayoutCursor::with_mainline(Some(names[0].clone())).with_lookahead(round % 2 * 8);
        let mut rows = layout(&nodes, &mut cursor);
        rows.extend(finish(&mut cursor));

        let mut chain = vec![0_usize];
        while let Some(parent) = spec[*chain.last().unwrap()].1.first() {
            chain.push(names.iter().position(|name| name == parent).unwrap());
        }
        let last = *chain.last().unwrap();

        for (index, row) in rows.iter().enumerate() {
            let on_chain = chain.contains(&index);
            assert_eq!(on_chain, row.lane == 0, "row {index}: {spec:?}");
            assert_eq!(on_chain, row.primary, "row {index}");
            if index > 0 && index < last && !on_chain {
                assert!(
                    row.segments.iter().any(|seg| seg.span == Span::Through
                        && seg.from == 0
                        && seg.to == 0
                        && seg.primary),
                    "row {index}: the main line is broken: {:?}\n{spec:?}",
                    row.segments
                );
            }
            for seg in &row.segments {
                if seg.span == Span::Through && (seg.from == 0 || seg.to == 0) {
                    assert!(seg.primary, "row {index}: something else crossed column 0");
                }
            }
        }
    }
}

/// Branches, merges, roots and hidden parents in random mixes, each laid out.
fn random_layouts() -> Vec<(Vec<CommitNode>, Vec<GraphRow>)> {
    let mut seed = 0x9e37_79b9_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };

    (0..300)
        .map(|round| {
            let count = 3 + next(40);
            let names: Vec<String> = (0..count).map(|i| format!("c{i}")).collect();
            let nodes: Vec<CommitNode> = (0..count)
                .map(|i| {
                    let older = count - i - 1;
                    let mut parents = Vec::new();
                    if older > 0 && next(6) != 0 {
                        parents.push(names[i + 1 + next(older.min(4))].clone());
                        if older > 1 && next(3) == 0 {
                            let other = names[i + 1 + next(older)].clone();
                            if !parents.contains(&other) {
                                parents.push(other);
                            }
                        }
                    }
                    let hidden = parents.iter().filter(|_| next(5) == 0).cloned().collect();
                    CommitNode {
                        oid: names[i].clone(),
                        parents,
                        hidden,
                    }
                })
                .collect();
            let primary = (next(2) == 0).then(|| names[next(count)].clone());
            // Every other history knows how long its lines are, as the app's does.
            let mut cursor = LayoutCursor::with_mainline(primary).with_lookahead(round % 2 * 8);
            let mut rows = layout(&nodes, &mut cursor);
            rows.extend(finish(&mut cursor));
            (nodes, rows)
        })
        .collect()
}

/// Only the node's own lines reach its ring: every other line keeps out of its column
/// halfway down, where the ring is; it may take the column over in the gaps.
#[test]
fn no_line_but_its_own_reaches_a_ring_on_random_histories() {
    for (nodes, rows) in random_layouts() {
        for (row, node) in rows.iter().zip(&nodes) {
            let parents = node
                .parents
                .iter()
                .filter(|parent| !node.hidden.contains(parent))
                .count();
            let out_of_ring = row
                .segments
                .iter()
                .filter(|seg| seg.span == Span::Bottom && !seg.arrow && seg.from == row.lane)
                .count();
            assert!(
                out_of_ring <= parents,
                "row {}: {:?}",
                row.row,
                row.segments
            );
            for seg in &row.segments {
                let passes_by = seg.span == Span::Through && seg.from == row.lane;
                assert!(!passes_by, "row {}: {seg:?} in {:?}", row.row, row.segments);
            }
        }
    }
}

/// One shape for every turn: half a row, as the node's own lines are. A lane only spans
/// the whole row where it runs straight.
#[test]
fn every_turn_takes_half_a_row_on_random_histories() {
    for (_, rows) in random_layouts() {
        for row in &rows {
            for seg in row.segments.iter().filter(|seg| seg.span == Span::Through) {
                assert_eq!(seg.from, seg.to, "row {}: {:?}", row.row, row.segments);
            }
        }
    }
}

/// The lane a merge pushes aside for its new line turns beside it, in the lower half.
#[test]
fn a_merge_line_crosses_a_lane_rather_than_pushing_it_aside() {
    let rows = run(
        &[
            ("top", &["m"]),
            ("t", &["z"]),
            ("m", &["a", "b"]),
            ("b", &["r"]),
            ("a", &["r"]),
            ("z", &["r"]),
            ("r", &[]),
        ],
        Some("m"),
    );
    let m = &rows[2];
    assert_eq!(
        segs(m, Span::Bottom),
        vec![(0, 0), (0, 2)],
        "{:?}",
        m.segments
    );
    assert_eq!(segs(m, Span::Through), vec![(1, 1)], "{:?}", m.segments);
}

#[test]
fn the_width_of_a_row_counts_both_edges_and_the_node() {
    let rows = run(
        &[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])],
        Some("m"),
    );
    assert_eq!(rows[0].width, 2, "the branch leaves in the lower half only");
    assert_eq!(rows[2].width, 2, "runs down its own column");
    assert_eq!(
        rows[3].width, 1,
        "and joins the line to its parent under its own commit"
    );
}

#[test]
fn a_parent_the_list_does_not_show_ends_in_an_arrow() {
    let mut nodes = commits(&[("c", &["b"]), ("a", &[])]);
    nodes[0].hidden = vec!["b".to_owned()];
    let mut cursor = LayoutCursor::with_mainline(None);

    let rows = layout(&nodes, &mut cursor);

    let arrow: Vec<_> = rows[0].segments.iter().filter(|seg| seg.arrow).collect();
    assert_eq!(arrow.len(), 1, "{:?}", rows[0].segments);
    assert_eq!(arrow[0].span, Span::Bottom);
    assert!(
        rows[1].segments.is_empty(),
        "nothing waits for b, so no line reaches the next row: {:?}",
        rows[1].segments
    );
}

// --- the rest of the model -------------------------------------------------------------

#[test]
fn a_tip_opens_right_beside_the_line_it_will_join() {
    let rows = run(
        &[
            ("m2", &["m1", "x0", "z1"]),
            ("y1", &["x0"]),
            ("z1", &["x0"]),
            ("m1", &["x0"]),
            ("x0", &[]),
        ],
        Some("m2"),
    );
    assert_eq!(
        rows[1].lane, 2,
        "y1 ends in x0: beside x0's line, the line there stepping aside"
    );
}

/// A tip beside the line it joins moves the lines right of it over for its row only.
#[test]
fn a_tip_moves_the_lines_right_of_it_over_for_its_row() {
    let rows = run(
        &[
            ("m2", &["m1", "x0", "z1"]),
            ("y1", &["x0"]),
            ("z1", &["x0"]),
            ("m1", &["x0"]),
            ("x0", &[]),
        ],
        Some("m2"),
    );
    let y1 = &rows[1];
    assert_eq!(
        segs(y1, Span::Through),
        vec![(0, 0), (1, 1)],
        "{:?}",
        y1.segments
    );
    assert_eq!(segs(y1, Span::Top), vec![(2, 3)]);
    assert!(segs(y1, Span::Bottom).contains(&(3, 2)));
}

/// The lines right of a root move left in the gap under it.
#[test]
fn a_root_frees_its_column_for_the_next_node() {
    let rows = run(
        &[
            ("a", &["ar"]),
            ("b", &["br"]),
            ("c", &["cr"]),
            ("br", &[]),
            ("cr", &[]),
            ("ar", &[]),
        ],
        None,
    );
    let br = &rows[3];
    assert_eq!(br.lane, 1);
    assert_eq!(segs(br, Span::Through), vec![(0, 0)], "{:?}", br.segments);
    assert_eq!(segs(br, Span::Bottom), vec![(2, 1)], "{:?}", br.segments);
    assert_eq!(rows[4].lane, 1);
    assert_eq!(segs(&rows[4], Span::Top), vec![(1, 1)]);
}

#[test]
fn a_line_moves_into_the_column_of_a_hidden_parents_arrow_below_it() {
    let mut nodes = commits(&[("t", &["b"]), ("u", &["up"]), ("b", &["h"]), ("up", &[])]);
    nodes[2].hidden = vec!["h".to_owned()];
    let mut cursor = LayoutCursor::with_mainline(None);

    let rows = layout(&nodes, &mut cursor);

    let b = &rows[2];
    assert!(b.segments.iter().any(|seg| seg.arrow && seg.from == 0));
    assert_eq!(
        segs(b, Span::Bottom),
        vec![(0, 0), (1, 0)],
        "{:?}",
        b.segments
    );
}

/// Straight down is the first parent's line; an arrow drawn over it would say that parent
/// is missing.
#[test]
fn a_hidden_second_parent_points_away_from_the_line_that_goes_on() {
    let mut nodes = commits(&[("m", &["a", "h"]), ("a", &[])]);
    nodes[0].hidden = vec!["h".to_owned()];
    let mut cursor = LayoutCursor::with_mainline(None);

    let rows = layout(&nodes, &mut cursor);

    let arrows: Vec<_> = rows[0].segments.iter().filter(|seg| seg.arrow).collect();
    assert_eq!(arrows.len(), 1, "{:?}", rows[0].segments);
    assert_eq!((arrows[0].from, arrows[0].to), (0, 1));
    assert_eq!(rows[0].width, 1, "a stub is shorter than a column");
}

/// A column a root gave back is taken by the line next to it in the gap under the root.
#[test]
fn a_freed_column_is_taken_back_in_the_gap() {
    let rows = run(
        &[
            ("a", &["ar"]),
            ("y", &["yp"]),
            ("n", &["np"]),
            ("ar", &[]),
            ("np", &[]),
        ],
        None,
    );
    assert_eq!(segs(&rows[3], Span::Bottom), vec![(1, 0), (2, 1)]);
    let np = &rows[4];
    assert_eq!(np.lane, 1);
    assert_eq!(segs(np, Span::Through), vec![(0, 0)], "{:?}", np.segments);
}

#[test]
fn a_branch_joins_the_line_to_its_parent_under_its_last_commit() {
    let rows = run(
        &[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])],
        Some("m"),
    );
    assert_eq!(rows[3].lane, 0);
    assert_eq!(segs(&rows[2], Span::Bottom), vec![(1, 0)]);
    assert_eq!(segs(&rows[3], Span::Top), vec![(0, 0)]);
}

#[test]
fn the_top_commit_takes_column_zero_whatever_the_primary_ref() {
    let rows = run(
        &[
            ("f2", &["f1"]),
            ("f1", &["m2"]),
            ("m2", &["m1"]),
            ("m1", &[]),
        ],
        Some("m2"),
    );
    assert!(rows.iter().all(|row| row.lane == 0), "one straight line");
    assert!(!rows[0].primary && !rows[1].primary);
    assert!(rows[2].primary && rows[3].primary);
    let into_head = rows[2].segments.iter().find(|seg| seg.span == Span::Top);
    assert!(
        into_head.is_some_and(|seg| !seg.primary),
        "the line above HEAD is not HEAD's"
    );
    assert!(
        rows[2]
            .segments
            .iter()
            .any(|seg| seg.span == Span::Bottom && seg.primary)
    );
}

#[test]
fn an_upstream_ahead_of_head_runs_straight_down_column_zero() {
    let rows = run(&[("o1", &["h"]), ("h", &["b"]), ("b", &[])], Some("h"));
    assert_eq!(
        rows.iter().map(|row| row.lane).collect::<Vec<_>>(),
        [0, 0, 0]
    );
    assert!(
        rows.iter().all(|row| row.width == 1),
        "no second column: {rows:?}"
    );
}

#[test]
fn the_main_line_goes_on_in_the_lane_its_parent_takes() {
    let rows = run(
        &[("d", &["x", "h"]), ("x", &["b"]), ("h", &["b"]), ("b", &[])],
        Some("h"),
    );
    assert_eq!(
        rows.iter().map(|row| row.lane).collect::<Vec<_>>(),
        [0, 0, 1, 0]
    );
    assert_eq!(
        rows.iter().map(|row| row.primary).collect::<Vec<_>>(),
        [false, false, true, true]
    );
    assert!(
        rows[2]
            .segments
            .iter()
            .any(|seg| seg.span == Span::Bottom && seg.primary)
    );
}

#[test]
fn column_zero_is_free_once_its_line_ends() {
    let rows = run(&[("m1", &[]), ("t2", &["t1"]), ("t1", &[])], Some("m1"));
    assert_eq!(
        rows.iter().map(|row| row.lane).collect::<Vec<_>>(),
        [0, 0, 0]
    );
}

#[test]
fn with_no_primary_ref_the_first_commit_takes_column_zero() {
    let rows = run(&[("c", &["b"]), ("b", &["a"]), ("a", &[])], None);
    assert!(rows.iter().all(|row| row.lane == 0));
    assert!(rows.iter().all(|row| !row.primary));
}

#[test]
fn a_finished_branch_gives_its_column_back() {
    let rows = run(
        &[
            ("m3", &["m2", "a1"]),
            ("a1", &["m1"]),
            ("m2", &["m1"]),
            ("m1", &["m0", "b1"]),
            ("b1", &["m0"]),
            ("m0", &[]),
        ],
        Some("m3"),
    );
    assert!(rows.iter().all(|row| row.width <= 2), "{rows:?}");
    assert_eq!(rows[4].lane, 1, "b reuses the column a left");
}

#[test]
fn independent_roots_are_never_joined() {
    let rows = run(&[("a", &[]), ("b", &[])], None);
    assert!(rows.iter().all(|row| row.segments.is_empty()), "{rows:?}");
    assert!(rows.iter().all(|row| row.kind == NodeKind::Root));
}

#[test]
fn a_root_and_a_merge_are_told_apart() {
    let rows = run(
        &[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])],
        Some("m"),
    );
    assert_eq!(rows[0].kind, NodeKind::Merge);
    assert_eq!(rows[1].kind, NodeKind::Normal);
    assert_eq!(rows[3].kind, NodeKind::Root);
}

#[test]
fn a_streamed_history_lays_out_exactly_as_it_would_all_at_once() {
    let all = commits(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    let mut whole = LayoutCursor::with_mainline(Some("m".to_owned()));
    let reference = layout(&all, &mut whole);

    let mut streamed = LayoutCursor::with_mainline(Some("m".to_owned()));
    let mut rows = layout(&all[..2], &mut streamed);
    rows.extend(layout(&all[2..], &mut streamed));

    assert_eq!(rows, reference);
}

#[test]
fn an_empty_chunk_lays_out_nothing() {
    let mut cursor = LayoutCursor::with_mainline(None);
    assert!(layout(&[], &mut cursor).is_empty());
}

// --- which ref is primary ------------------------------------------------------------------

#[test]
fn the_branch_head_is_on_takes_the_column_over_master() {
    let tip = mainline_tip(
        &[("feature", "f1"), ("master", "m1"), ("topic", "t1")],
        Some("f1"),
    );
    assert_eq!(tip.as_deref(), Some("f1"));
}

#[test]
fn master_is_the_mainline_when_there_is_no_head_to_follow() {
    let tip = mainline_tip(&[("feature", "f1"), ("master", "m1")], None);
    assert_eq!(tip.as_deref(), Some("m1"));
}

#[test]
fn main_counts_the_same_as_master() {
    let tip = mainline_tip(&[("feature", "f1"), ("main", "m1")], None);
    assert_eq!(tip.as_deref(), Some("m1"));
}

/// Both names exist in repositories that were renamed and kept the old branch around.
#[test]
fn master_wins_over_main_rather_than_picking_at_random() {
    let tip = mainline_tip(&[("main", "a"), ("master", "b")], None);
    assert_eq!(tip.as_deref(), Some("b"));
}

#[test]
fn a_repository_with_no_branches_at_all_names_no_mainline() {
    assert_eq!(mainline_tip(&[], None), None);
}

#[test]
fn a_detached_head_is_its_own_mainline() {
    let tip = mainline_tip(&[("topic", "t1")], Some("deadbeef"));
    assert_eq!(tip.as_deref(), Some("deadbeef"));
}

#[test]
fn a_stash_is_three_nodes_each_line_bending_into_its_parent() {
    let rows = run(
        &[
            ("s", &["h", "i", "u"]),
            ("i", &["h"]),
            ("u", &[]),
            ("h", &["g"]),
            ("g", &[]),
        ],
        Some("h"),
    );
    assert_eq!(rows[0].kind, NodeKind::Merge);
    assert_eq!(segs(&rows[0], Span::Bottom), vec![(0, 0), (0, 1), (0, 2)]);
    assert!(segs(&rows[0], Span::Top).is_empty());
    assert_eq!(rows.len(), 5);
    assert_eq!(
        segs(&rows[1], Span::Bottom),
        vec![(1, 0), (2, 1)],
        "{:?}",
        rows[1].segments
    );
    assert_eq!(rows[2].kind, NodeKind::Root);
    assert_eq!(segs(&rows[3], Span::Top), vec![(0, 0)]);
}

#[test]
fn branches_off_one_parent_join_its_line_under_their_own_commits() {
    let rows = run(
        &[
            ("m", &["p"]),
            ("a2", &["a1"]),
            ("b1", &["p"]),
            ("a1", &["p"]),
            ("p", &[]),
        ],
        Some("m"),
    );
    assert_eq!(
        segs(&rows[2], Span::Bottom),
        vec![(1, 0), (2, 1)],
        "{:?}",
        rows[2].segments
    );
    assert_eq!(
        segs(&rows[3], Span::Bottom),
        vec![(1, 0)],
        "{:?}",
        rows[3].segments
    );
    assert_eq!(segs(&rows[4], Span::Top), vec![(0, 0)]);
}

// --- every edge reaches its parent, through the gaps and the lines it joins ----------------

/// Every visible edge, traced from the child's row down: through the rows it passes, at
/// the column its `Bottom` reaches in each gap, into the line it joins, until it ends in a
/// node; that node is one of the child's parents, and every parent is reached.
fn assert_edges(nodes: &[CommitNode], rows: &[GraphRow]) {
    for (index, (row, node)) in rows.iter().zip(nodes).enumerate() {
        let visible: Vec<&String> = node
            .parents
            .iter()
            .filter(|parent| !node.hidden.contains(parent))
            .collect();
        let mut reached = Vec::new();
        for start in row
            .segments
            .iter()
            .filter(|seg| seg.span == Span::Bottom && !seg.arrow && seg.from == row.lane)
        {
            let mut column = start.to;
            let mut parent = None;
            for (below, next) in rows.iter().enumerate().skip(index + 1) {
                let Some(seg) = next
                    .segments
                    .iter()
                    .find(|seg| !seg.arrow && seg.from == column && seg.span != Span::Bottom)
                else {
                    panic!("row {}: the line in column {column} breaks off", next.row);
                };
                if seg.span == Span::Through {
                    continue;
                }
                if seg.to == next.lane {
                    parent = Some(&nodes[below].oid);
                    break;
                }
                let Some(down) = next
                    .segments
                    .iter()
                    .find(|down| !down.arrow && down.span == Span::Bottom && down.from == seg.to)
                else {
                    panic!("row {}: the line at column {} stops", next.row, seg.to);
                };
                column = down.to;
            }
            reached.push(parent.expect("the line reaches a node"));
        }
        reached.sort();
        reached.dedup();
        let mut wanted = visible.clone();
        wanted.sort();
        assert_eq!(reached, wanted, "the edges of {}", node.oid);
    }
}

#[test]
fn a_commit_forked_from_goes_on_straight_up_to_its_main_child() {
    let spec: &[(&str, &[&str])] = &[
        ("top", &["merge"]),
        ("side", &["merge"]),
        ("merge", &["base", "fix"]),
        ("fix", &["base"]),
        ("base", &[]),
    ];
    let rows = run(spec, Some("top"));
    assert_edges(&commits(spec), &rows);
    assert_eq!((rows[0].lane, rows[2].lane), (0, 0));
    assert_eq!(rows[1].lane, 1);
    assert_eq!(
        segs(&rows[1], Span::Bottom),
        vec![(1, 0)],
        "side joins the line to merge under itself"
    );
    assert_eq!(segs(&rows[2], Span::Top), vec![(0, 0)]);
}

/// The real case: HEAD elsewhere, so neither child is on column 0, and the merge's first
/// parent is the libjam commit. The fork commit stays under the child it is first parent of.
#[test]
fn off_the_main_line_a_fork_has_no_step_between_two_children() {
    let spec: &[(&str, &[&str])] = &[
        ("head", &["h0"]),
        ("m35", &["libjam", "can"]),
        ("can", &["m27"]),
        ("libjam", &["m27"]),
        ("m27", &["h0"]),
        ("h0", &[]),
    ];
    let rows = run(spec, Some("head"));
    assert_edges(&commits(spec), &rows);
    assert_eq!(rows[3].lane, rows[4].lane, "libjam and m27 share a column");
    assert_eq!(rows[1].lane, rows[3].lane);
}

#[test]
fn several_branches_off_one_commit_each_bend_out_of_it() {
    let spec: &[(&str, &[&str])] = &[
        ("m", &["p"]),
        ("a2", &["a1"]),
        ("b1", &["p"]),
        ("a1", &["p"]),
        ("c1", &["p"]),
        ("p", &[]),
    ];
    let rows = run(spec, Some("m"));
    assert_edges(&commits(spec), &rows);
    assert_eq!(rows[5].lane, 0);
}

#[test]
fn a_branch_merged_back_keeps_both_lines_straight() {
    let spec: &[(&str, &[&str])] = &[
        ("m3", &["m2", "b2"]),
        ("b2", &["b1"]),
        ("m2", &["m1"]),
        ("b1", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ];
    assert_edges(&commits(spec), &run(spec, Some("m3")));
}

#[test]
fn a_merge_of_two_long_branches_keeps_both_lines_straight() {
    let mut spec: Vec<(String, Vec<String>)> = vec![("m".into(), vec!["a9".into(), "b9".into()])];
    for i in (0..10).rev() {
        for side in ["a", "b"] {
            let parent = if i == 0 {
                "r".to_owned()
            } else {
                format!("{side}{}", i - 1)
            };
            spec.push((format!("{side}{i}"), vec![parent]));
        }
    }
    spec.push(("r".into(), vec![]));
    let nodes: Vec<CommitNode> = spec
        .iter()
        .map(|(oid, parents)| CommitNode {
            oid: oid.clone(),
            parents: parents.clone(),
            hidden: vec![],
        })
        .collect();
    let rows = layout(&nodes, &mut LayoutCursor::with_mainline(Some("m".into())));
    assert_edges(&nodes, &rows);
}

#[test]
fn an_octopus_bends_each_line_out_of_its_node() {
    let spec: &[(&str, &[&str])] = &[
        ("o", &["m", "a", "b", "c"]),
        ("c", &["r"]),
        ("b", &["r"]),
        ("a", &["r"]),
        ("m", &["r"]),
        ("r", &[]),
    ];
    assert_edges(&commits(spec), &run(spec, Some("o")));
}

/// A column freed left of a line that goes on is taken by it in the gap.
#[test]
fn a_column_closing_left_of_a_line_takes_that_line_over() {
    let spec: &[(&str, &[&str])] = &[
        ("m2", &["m1", "a1"]),
        ("b2", &["b1"]),
        ("a1", &["m1"]),
        ("x", &["m1"]),
        ("b1", &["m0"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ];
    let rows = run(spec, Some("m2"));
    assert_edges(&commits(spec), &rows);
    assert_eq!((rows[1].lane, rows[4].lane), (2, 1), "b moves left");
}

#[test]
fn every_edge_reaches_its_parent_on_random_histories() {
    for (nodes, rows) in random_layouts() {
        assert_edges(&nodes, &rows);
    }
}
