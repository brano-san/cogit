// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Paint over a finished layout: lanes followed by their columns, branch colours along
//! first parents. The layout itself is never touched.

use std::collections::HashMap;

use graph_engine::{
    CommitNode, GraphRow, LayoutCursor, PAINT_SLOT, Paint, PaintSpec, Span, layout, paint,
};

fn nodes(spec: &[(&str, &[&str])]) -> Vec<CommitNode> {
    spec.iter()
        .map(|(oid, parents)| CommitNode {
            oid: (*oid).to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            hidden: Vec::new(),
        })
        .collect()
}

/// Parents as laid out: hidden ones left out, a parent the list lacks as `None`.
fn parent_rows(nodes: &[CommitNode]) -> Vec<Vec<Option<u32>>> {
    let at: HashMap<&str, u32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.oid.as_str(), u32::try_from(i).unwrap()))
        .collect();
    nodes
        .iter()
        .map(|n| {
            n.parents
                .iter()
                .filter(|p| !n.hidden.contains(p))
                .map(|p| at.get(p.as_str()).copied())
                .collect()
        })
        .collect()
}

struct Painted {
    rows: Vec<GraphRow>,
    paint: Paint,
}

impl Painted {
    fn new(nodes: &[CommitNode], mainline: Option<&str>, spec: &PaintSpec) -> Self {
        let rows = layout(
            nodes,
            &mut LayoutCursor::with_mainline(mainline.map(str::to_owned)),
        );
        let paint = paint(&rows, &parent_rows(nodes), spec);
        Self { rows, paint }
    }

    /// Row, segment and the segment's style.
    fn segments(&self) -> impl Iterator<Item = (usize, &graph_engine::Segment, u8, u32)> {
        self.rows.iter().enumerate().flat_map(move |(r, row)| {
            let first = self.paint.segment_first[r] as usize;
            row.segments.iter().enumerate().map(move |(i, s)| {
                (
                    r,
                    s,
                    self.paint.segment_style[first + i],
                    self.paint.segment_lane[first + i],
                )
            })
        })
    }
}

fn tips(list: &[(u32, u8)]) -> PaintSpec {
    PaintSpec {
        tips: list.to_vec(),
    }
}

#[test]
fn with_nothing_to_paint_everything_keeps_the_default_style() {
    let history = nodes(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
    let painted = Painted::new(&history, Some("c"), &PaintSpec::default());
    assert!(painted.paint.node_style.iter().all(|s| *s == 0));
    assert!(painted.paint.segment_style.iter().all(|s| *s == 0));
    assert_eq!(painted.paint.segment_first.len(), history.len() + 1);
}

#[test]
fn a_ticked_branch_is_coloured_from_its_tip_to_the_main_line() {
    let history = nodes(&[
        ("m4", &["m3"]),
        ("m3", &["m2", "t1"]),
        ("t1", &["m1"]),
        ("m2", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("m4"), &tips(&[(2, 4)]));

    assert_eq!(painted.paint.node_style, vec![0, 0, 5, 0, 0, 0]);
    for (row, segment, style, _) in painted.segments() {
        let expected = if segment.primary { 0 } else { 5 };
        assert_eq!(style, expected, "row {row}: {segment:?}");
    }
}

#[test]
fn a_branch_stops_where_it_meets_a_line_already_coloured() {
    let history = nodes(&[
        ("m2", &["m1"]),
        ("a2", &["a1"]),
        ("b1", &["a1"]),
        ("a1", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("m2"), &tips(&[(2, 1), (1, 0)]));

    assert_eq!(painted.paint.node_style, vec![0, 1, 2, 1, 0, 0]);
    let into_a1: Vec<u8> = painted
        .segments()
        .filter(|(row, s, _, _)| *row == 3 && s.span == Span::Top)
        .map(|(_, _, style, _)| style)
        .collect();
    let mut sorted = into_a1.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![1, 2],
        "a2 and b1 each bring their colour into a1"
    );
}

#[test]
fn a_branch_goes_on_through_a_line_nobody_ticked() {
    let history = nodes(&[
        ("m3", &["m2", "s2"]),
        ("b1", &["s1"]),
        ("s2", &["s1"]),
        ("m2", &["m1"]),
        ("s1", &["m1"]),
        ("m1", &[]),
    ]);
    let painted = Painted::new(&history, Some("m3"), &tips(&[(1, 3)]));

    assert_eq!(painted.paint.node_style, vec![0, 4, 0, 0, 4, 0]);
    let mut into_s1: Vec<u8> = painted
        .segments()
        .filter(|(row, s, _, _)| *row == 4 && s.span == Span::Top)
        .map(|(_, _, style, _)| style)
        .collect();
    into_s1.sort_unstable();
    assert_eq!(into_s1, vec![0, 4], "only b1's line into s1 is its colour");
    for (row, s, style, _) in painted.segments() {
        if row == 4 && s.span == Span::Bottom {
            assert_eq!(style, 4, "s1 goes on in b1's colour: {s:?}");
        }
    }
}

#[test]
fn a_tip_on_the_main_line_colours_nothing() {
    let history = nodes(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
    let painted = Painted::new(&history, Some("c"), &tips(&[(1, 2)]));
    assert!(painted.paint.node_style.iter().all(|s| *s == 0));
}

#[test]
fn a_slot_past_the_palette_is_clamped_to_the_slot_bits() {
    let history = nodes(&[("m1", &["m0"]), ("t", &["m0"]), ("m0", &[])]);
    let painted = Painted::new(&history, Some("m1"), &tips(&[(1, 200)]));
    assert_eq!(painted.paint.node_style[1], PAINT_SLOT);
}

/// Branches, merges, roots and hidden parents in random mixes.
fn random_histories() -> Vec<(Vec<CommitNode>, Option<String>)> {
    let mut seed = 0x51f1_5eed_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };
    (0..300)
        .map(|_| {
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
            (nodes, primary)
        })
        .collect()
}

/// The lanes found from the columns are the layout's own: each lane's nodes are one
/// first-parent chain, and a lane has one colour, the one the layout gave it.
#[test]
fn traced_lanes_are_the_layouts_lanes_on_random_histories() {
    for (history, primary) in random_histories() {
        let painted = Painted::new(&history, primary.as_deref(), &PaintSpec::default());
        let parents = parent_rows(&history);
        let mut colour: HashMap<u32, u8> = HashMap::new();
        let mut last: HashMap<u32, usize> = HashMap::new();
        for (r, row) in painted.rows.iter().enumerate() {
            let lane = painted.paint.node_lane[r];
            assert_eq!(
                *colour.entry(lane).or_insert(row.color),
                row.color,
                "row {r}: {history:?}"
            );
            if let Some(&above) = last.get(&lane) {
                assert_eq!(
                    parents[above].first().copied().flatten(),
                    Some(u32::try_from(r).unwrap()),
                    "lane {lane}: row {r} is not the first parent of row {above}\n{history:?}"
                );
            }
            last.insert(lane, r);
        }
        for (r, s, _, lane) in painted.segments() {
            assert_eq!(
                *colour.entry(lane).or_insert(s.color),
                s.color,
                "row {r}: {s:?} in {:?}\n{history:?}",
                painted.rows[r].segments
            );
        }
    }
}

#[test]
fn a_node_goes_on_in_its_own_lane_on_random_histories() {
    for (history, primary) in random_histories() {
        let painted = Painted::new(&history, primary.as_deref(), &PaintSpec::default());
        let parents = parent_rows(&history);
        for (r, row) in painted.rows.iter().enumerate() {
            if parents[r].is_empty() {
                continue;
            }
            let own = painted.segments().any(|(at, s, _, lane)| {
                at == r
                    && s.span == Span::Bottom
                    && s.from == row.lane
                    && (s.to == row.lane || !s.arrow)
                    && lane == painted.paint.node_lane[r]
            });
            assert!(own, "row {r}: {:?}\n{history:?}", row.segments);
        }
    }
}
