// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Long links cut into two stubs, as SmartGit draws them (R-330): the column they would
//! hold goes back to the lanes, and the main line is never cut.

use graph_engine::{CommitNode, GraphRow, LayoutCursor, Span, finish, layout, push};

fn commits(spec: &[(&str, &[&str])]) -> Vec<CommitNode> {
    spec.iter()
        .map(|(oid, parents)| CommitNode {
            oid: (*oid).to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            hidden: Vec::new(),
        })
        .collect()
}

fn cut(nodes: Vec<CommitNode>, mainline: Option<&str>, rows: u32) -> Vec<GraphRow> {
    let mut cursor = LayoutCursor::with_mainline(mainline.map(str::to_owned)).with_long_links(rows);
    let mut out = push(nodes, &mut cursor);
    out.extend(finish(&mut cursor));
    out
}

/// The far end of each stub in `row`, with the stub's span.
fn ends(row: &GraphRow) -> Vec<(Span, String)> {
    row.links
        .iter()
        .map(|link| {
            (
                row.segments[usize::from(link.segment)].span,
                link.oid.clone(),
            )
        })
        .collect()
}

/// A feature branch forked six commits down the main line.
const FORKED: &[(&str, &[&str])] = &[
    ("m6", &["m5"]),
    ("f1", &["m0"]),
    ("m5", &["m4"]),
    ("m4", &["m3"]),
    ("m3", &["m2"]),
    ("m2", &["m1"]),
    ("m1", &["m0"]),
    ("m0", &[]),
];

#[test]
fn a_link_longer_than_the_limit_is_two_stubs_and_its_column_goes_back() {
    let whole = cut(commits(FORKED), Some("m6"), 0);
    assert!(
        whole[2..7].iter().all(|row| row.width == 2),
        "the branch holds column 1 all the way down"
    );

    let rows = cut(commits(FORKED), Some("m6"), 2);

    assert_eq!(rows.len(), FORKED.len());
    assert_eq!(
        ends(&rows[1]),
        vec![(Span::Bottom, "m0".to_owned())],
        "{:?}",
        rows[1]
    );
    assert_eq!(
        ends(&rows[7]),
        vec![(Span::Top, "f1".to_owned())],
        "{:?}",
        rows[7]
    );
    assert!(rows[2..7].iter().all(|row| row.width == 1), "{rows:?}");
    let stub = &rows[1].segments[usize::from(rows[1].links[0].segment)];
    assert!(stub.arrow && stub.from == stub.to && stub.from == rows[1].lane);
}

#[test]
fn a_link_within_the_limit_is_drawn_whole() {
    let rows = cut(commits(FORKED), Some("m6"), 6);

    assert!(rows.iter().all(|row| row.links.is_empty()), "{rows:?}");
    assert_eq!(rows, cut(commits(FORKED), Some("m6"), 0));
}

#[test]
fn the_main_line_is_never_cut() {
    let rows = cut(
        commits(&[
            ("m1", &["m0"]),
            ("a", &["ar"]),
            ("b", &["br"]),
            ("c", &["cr"]),
            ("ar", &[]),
            ("br", &[]),
            ("cr", &[]),
            ("m0", &[]),
        ]),
        Some("m1"),
        2,
    );

    assert!(rows[0].links.is_empty(), "{:?}", rows[0]);
    for row in &rows[1..7] {
        assert!(
            row.segments
                .iter()
                .any(|seg| seg.span == Span::Through && seg.from == 0 && seg.primary),
            "row {}: {:?}",
            row.row,
            row.segments
        );
    }
    assert_eq!(rows[7].lane, 0);
}

#[test]
fn a_far_second_parent_leans_away_from_the_first_parent_line() {
    let rows = cut(
        commits(&[
            ("m", &["a", "x"]),
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["x"]),
            ("x", &[]),
        ]),
        Some("m"),
        2,
    );

    assert_eq!(ends(&rows[0]), vec![(Span::Bottom, "x".to_owned())]);
    let stub = &rows[0].segments[usize::from(rows[0].links[0].segment)];
    assert_eq!((stub.from, stub.to), (0, 1));
    assert_eq!(rows[0].width, 1, "no lane opens for x");
}

#[test]
fn a_second_parent_already_on_its_way_joins_its_lane_rather_than_being_cut() {
    let rows = cut(
        commits(&[
            ("m", &["y"]),
            ("s", &["sa", "y"]),
            ("sa", &[]),
            ("q1", &[]),
            ("q2", &[]),
            ("y", &[]),
        ]),
        Some("m"),
        2,
    );

    let s = &rows[1];
    assert!(s.links.is_empty(), "{s:?}");
    assert!(
        s.segments
            .iter()
            .any(|seg| seg.span == Span::Bottom && (seg.from, seg.to) == (1, 0)),
        "it joins the main line waiting for y: {s:?}"
    );
}

#[test]
fn a_parent_several_long_links_meet_in_lists_the_nearest_child_first() {
    let rows = cut(
        commits(&[
            ("m4", &["m3"]),
            ("f", &["m0"]),
            ("g", &["m0"]),
            ("m3", &["m2"]),
            ("m2", &["m1"]),
            ("m1", &["m0"]),
            ("m0", &[]),
        ]),
        Some("m4"),
        2,
    );

    assert_eq!(
        ends(&rows[6]),
        vec![(Span::Top, "g".to_owned()), (Span::Top, "f".to_owned())],
        "one stub, both ends: {:?}",
        rows[6]
    );
    assert_eq!(rows[6].segments.iter().filter(|seg| seg.arrow).count(), 1);
}

#[test]
fn rows_held_back_come_out_in_order_whatever_the_chunks() {
    let reference = cut(commits(FORKED), Some("m6"), 2);

    let mut cursor = LayoutCursor::with_mainline(Some("m6".to_owned())).with_long_links(2);
    let nodes = commits(FORKED);
    let mut rows = Vec::new();
    for chunk in nodes.chunks(3) {
        let got = push(chunk.to_vec(), &mut cursor);
        assert!(got.len() <= chunk.len());
        rows.extend(got);
    }
    assert_eq!(
        rows.len(),
        FORKED.len() - 2,
        "two rows wait for what comes after them"
    );
    rows.extend(finish(&mut cursor));

    assert_eq!(rows, reference);
    let mut sliced = LayoutCursor::with_mainline(Some("m6".to_owned())).with_long_links(2);
    let mut at_once = layout(&nodes, &mut sliced);
    at_once.extend(finish(&mut sliced));
    assert_eq!(at_once, reference);
}

/// On any history: column 0 stays the unbroken main line, only long links are cut, and
/// every stub has its partner at the other end.
#[test]
fn cut_links_pair_up_and_leave_the_main_line_alone_on_random_histories() {
    let mut seed = 0x51f1_5eed_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };

    for _ in 0..300 {
        let count = 3 + next(60);
        let names: Vec<String> = (0..count).map(|i| format!("c{i}")).collect();
        let nodes: Vec<CommitNode> = (0..count)
            .map(|i| {
                let older = count - i - 1;
                let mut parents = Vec::new();
                if older > 0 && next(8) != 0 {
                    let reach = if next(4) == 0 { older } else { older.min(3) };
                    parents.push(names[i + 1 + next(reach)].clone());
                    if older > 1 && next(3) == 0 {
                        let other = names[i + 1 + next(older)].clone();
                        if !parents.contains(&other) {
                            parents.push(other);
                        }
                    }
                }
                CommitNode {
                    oid: names[i].clone(),
                    parents,
                    hidden: Vec::new(),
                }
            })
            .collect();
        let limit = 1 + u32::try_from(next(6)).unwrap();
        let rows = cut(nodes.clone(), Some(&names[0]), limit);
        assert_eq!(rows.len(), count);

        let mut chain = vec![0_usize];
        while let Some(parent) = nodes[*chain.last().unwrap()].parents.first() {
            chain.push(names.iter().position(|name| name == parent).unwrap());
        }
        let last = *chain.last().unwrap();
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                chain.contains(&index),
                row.lane == 0,
                "row {index}: {nodes:?}"
            );
            if index > 0 && index < last && !chain.contains(&index) {
                assert!(
                    row.segments
                        .iter()
                        .any(|seg| seg.span == Span::Through && seg.from == 0 && seg.primary),
                    "row {index}: the main line is broken"
                );
            }
            for seg in &row.segments {
                let passes_by = match seg.span {
                    Span::Through => seg.from == row.lane || seg.to == row.lane,
                    Span::Bottom => seg.to == row.lane && seg.from != row.lane,
                    Span::Top => false,
                };
                assert!(!passes_by, "row {index}: {seg:?} reaches the ring");
            }
            for link in &row.links {
                let segment = &row.segments[usize::from(link.segment)];
                assert!(
                    segment.arrow && !segment.primary,
                    "row {index}: {segment:?}"
                );
                let other = names.iter().position(|name| *name == link.oid).unwrap();
                let (child, parent) = if segment.span == Span::Bottom {
                    (index, other)
                } else {
                    (other, index)
                };
                assert!(
                    parent > child + usize::try_from(limit).unwrap(),
                    "row {index}: a short link was cut"
                );
                assert!(
                    rows[other]
                        .links
                        .iter()
                        .any(|back| back.oid == names[index]),
                    "row {index}: {} has no stub back",
                    link.oid
                );
            }
        }
    }
}
