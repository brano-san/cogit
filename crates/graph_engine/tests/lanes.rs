// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The SmartGit layout: an ordered list of active lanes, re-indexed on every row. A lane
//! that ends lets every lane to its right slide left within that row; only lane 0, the
//! first-parent chain of the primary ref, never moves (doc/07-graph-rendering.md).

use graph_engine::{CommitNode, GraphRow, LayoutCursor, NodeKind, Span, layout, mainline_tip};

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
fn a_branch_merged_back_has_two_lanes_while_it_lives_and_the_main_one_never_moves() {
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
    assert_eq!(widths, vec![1, 2, 2, 2, 2, 1]);
    assert_eq!(
        segs(&rows[1], Span::Bottom),
        vec![(0, 0), (0, 1)],
        "t1 leaves the merge"
    );
    assert_eq!(rows[2].lane, 1, "the branch sits right of the main line");
    assert_eq!(
        segs(&rows[4], Span::Top),
        vec![(0, 0), (1, 0)],
        "and curves into its fork"
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
fn a_lane_ending_in_the_middle_slides_the_lanes_right_of_it_left_in_that_row() {
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

    let m1 = &rows[4];
    assert_eq!(
        segs(m1, Span::Top),
        vec![(0, 0), (1, 0)],
        "a's lane ends in m1"
    );
    assert!(
        segs(m1, Span::Through).contains(&(2, 1)),
        "b slides from column 2 to 1 within the row: {:?}",
        m1.segments
    );
    assert_eq!(rows[5].lane, 1, "and carries on from its new column");
}

#[test]
fn an_octopus_opens_its_new_lanes_right_of_the_node_in_parent_order() {
    let rows = run(
        &[
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

    let m = &rows[1];
    assert_eq!(m.lane, 0);
    assert_eq!(segs(m, Span::Bottom), vec![(0, 0), (0, 1), (0, 2)]);
    assert!(
        segs(m, Span::Through).contains(&(1, 3)),
        "the lane already there makes room rather than the new ones going far right: {:?}",
        m.segments
    );
    assert_eq!(rows[3].lane, 1, "b takes the first new column");
    assert_eq!(rows[4].lane, 2, "c the second");
}

/// Requirement 5: on any history, column 0 is the first-parent chain of the primary ref,
/// unbroken, and nothing else.
#[test]
fn the_main_line_holds_column_zero_on_random_histories() {
    let mut seed = 0x2545_f491_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };

    for _ in 0..300 {
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
        let mut cursor = LayoutCursor::with_mainline(Some(names[0].clone()));
        let rows = layout(&nodes, &mut cursor);

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

#[test]
fn the_width_of_a_row_counts_both_edges_and_the_node() {
    let rows = run(
        &[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])],
        Some("m"),
    );
    assert_eq!(rows[0].width, 2, "the branch leaves in the lower half only");
    assert_eq!(rows[3].width, 2, "and comes back in the upper half only");
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
fn a_tip_is_placed_beside_the_lane_it_will_join() {
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
        "y1 ends in x0, so it opens beside the lane waiting for x0"
    );
    assert!(
        segs(&rows[1], Span::Through).contains(&(2, 3)),
        "{:?}",
        rows[1].segments
    );
}

#[test]
fn a_commit_waited_for_by_several_lanes_is_one_node_where_they_meet() {
    let rows = run(
        &[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])],
        Some("m"),
    );
    assert_eq!(rows[3].lane, 0);
    assert_eq!(segs(&rows[3], Span::Top), vec![(0, 0), (1, 0)]);
}

#[test]
fn the_main_column_waits_empty_until_the_primary_ref_arrives() {
    let rows = run(
        &[
            ("f2", &["f1"]),
            ("f1", &["m2"]),
            ("m2", &["m1"]),
            ("m1", &[]),
        ],
        Some("m2"),
    );
    assert_ne!(rows[0].lane, 0, "a newer feature branch gives way");
    assert!(
        rows[0]
            .segments
            .iter()
            .all(|seg| seg.from != 0 && seg.to != 0),
        "and nothing is drawn in the empty main column: {:?}",
        rows[0].segments
    );
    assert_eq!(rows[2].lane, 0);
    assert_eq!(rows[3].lane, 0);
}

#[test]
fn the_main_column_stays_reserved_across_a_chunk_boundary() {
    let mut cursor = LayoutCursor::with_mainline(Some("m1".to_owned()));
    let first = layout(&commits(&[("f2", &["f1"]), ("f1", &["m1"])]), &mut cursor);
    let second = layout(&commits(&[("m1", &[])]), &mut cursor);

    assert!(first.iter().all(|row| row.lane != 0));
    assert_eq!(second[0].lane, 0);
    assert_eq!(second[0].row, 2, "rows keep counting across chunks");
}

#[test]
fn column_zero_is_not_given_away_after_the_main_line_ends() {
    let rows = run(&[("m1", &[]), ("t2", &["t1"]), ("t1", &[])], Some("m1"));
    assert_eq!(rows[0].lane, 0);
    assert_ne!(rows[1].lane, 0);
    assert_ne!(rows[2].lane, 0);
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
