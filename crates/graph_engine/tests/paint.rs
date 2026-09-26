// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Paint over a finished layout: lanes followed by their columns, branch colours along
//! first parents, ancestry dimming. The layout itself is never touched.

use std::collections::HashMap;

use graph_engine::{
    CommitNode, GraphRow, LayoutCursor, PAINT_DIM, PAINT_SLOT, Paint, PaintSpec, Span, finish,
    paint, push,
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
        Self::cut(nodes, mainline, 0, spec)
    }

    /// Laid out with links longer than `long` rows cut into stubs.
    fn cut(nodes: &[CommitNode], mainline: Option<&str>, long: u32, spec: &PaintSpec) -> Self {
        let mut cursor =
            LayoutCursor::with_mainline(mainline.map(str::to_owned)).with_long_links(long);
        let mut rows = push(nodes.to_vec(), &mut cursor);
        rows.extend(finish(&mut cursor));
        let at: HashMap<&str, u32> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.oid.as_str(), u32::try_from(i).unwrap()))
            .collect();
        let paint = paint(
            &rows,
            &parent_rows(nodes),
            &|oid| at.get(oid).copied(),
            spec,
        );
        Self { rows, paint }
    }

    /// Whether segment `index` of `row` is a stub of a cut link.
    fn stub(&self, row: usize, index: usize) -> bool {
        self.rows[row]
            .links
            .iter()
            .any(|link| usize::from(link.segment) == index)
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
        ..PaintSpec::default()
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
        ("m3", &["m2"]),
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

/// A feature branched off and merged back is all history of the main line: a tick on it
/// colours nothing, as none of it is its own any more (#21).
#[test]
fn a_feature_the_main_line_merged_colours_nothing() {
    let history = nodes(&[
        ("m4", &["m3"]),
        ("m3", &["m2", "t1"]),
        ("t1", &["m1"]),
        ("m2", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("m4"), &tips(&[(2, 4)]));

    assert!(painted.paint.node_style.iter().all(|s| *s == 0));
    for (row, segment, style, _) in painted.segments() {
        assert_eq!(style, 0, "row {row}: {segment:?}");
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
        ("m3", &["m2"]),
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

/// What the main line has merged is common history, whoever ticked the branch it came
/// from: the line stays the colour it has without a tick.
#[test]
fn a_branch_stops_at_history_the_main_line_merged() {
    let history = nodes(&[
        ("m3", &["m2", "s2"]),
        ("b1", &["s1"]),
        ("s2", &["s1"]),
        ("m2", &["m1"]),
        ("s1", &["m1"]),
        ("m1", &[]),
    ]);
    let painted = Painted::new(&history, Some("m3"), &tips(&[(1, 3)]));

    assert_eq!(painted.paint.node_style, vec![0, 4, 0, 0, 0, 0]);
    let mut into_s1: Vec<u8> = painted
        .segments()
        .filter(|(row, s, _, _)| *row == 4 && s.span == Span::Top)
        .map(|(_, _, style, _)| style)
        .collect();
    into_s1.sort_unstable();
    assert_eq!(into_s1, vec![0, 4], "b1's line runs into s1 in its colour");
    for (row, s, style, _) in painted.segments() {
        if row == 4 && s.span == Span::Bottom {
            assert_eq!(style, 0, "s1 is the main line's history: {s:?}");
        }
    }
}

/// `master` merged into the checked-out feature: all of it is the feature's history too,
/// so ticking it colours nothing, down to the root (#21).
#[test]
fn a_ticked_branch_the_main_line_merged_whole_colours_nothing() {
    let history = nodes(&[
        ("f3", &["f2", "m2"]),
        ("m2", &["m1"]),
        ("f2", &["f1"]),
        ("m1", &["m0"]),
        ("f1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("f3"), &tips(&[(1, 4)]));

    assert!(painted.paint.node_style.iter().all(|s| *s == 0));
    for (row, s, style, _) in painted.segments() {
        assert_eq!(style, 0, "row {row}: {s:?}");
    }
}

/// `master` went on after the feature merged it: only the commits since are its own, and
/// the merge's line into the part merged stays uncoloured though it shares a column.
#[test]
fn only_what_the_main_line_has_not_merged_takes_the_colour() {
    let history = nodes(&[
        ("m3", &["m2"]),
        ("f3", &["f2", "m2"]),
        ("m2", &["m1"]),
        ("f2", &["f1"]),
        ("m1", &["m0"]),
        ("f1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("f3"), &tips(&[(0, 4)]));

    assert_eq!(painted.paint.node_style, vec![5, 0, 0, 0, 0, 0, 0]);
    let coloured: Vec<(usize, Span, u16, u16)> = painted
        .segments()
        .filter(|(_, _, style, _)| *style != 0)
        .map(|(row, s, _, _)| (row, s.span, s.from, s.to))
        .collect();
    assert_eq!(
        coloured,
        vec![
            (0, Span::Bottom, 1, 1),
            (1, Span::Through, 1, 1),
            (2, Span::Top, 1, 1),
        ],
        "m3's line down to m2, not f3's merge into it"
    );
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

#[test]
fn ancestry_dims_what_is_neither_ancestor_nor_descendant() {
    let history = nodes(&[
        ("e", &["d"]),
        ("d", &["b", "c"]),
        ("b", &["a"]),
        ("c", &["a"]),
        ("a", &[]),
    ]);
    let spec = PaintSpec {
        ancestry_of: Some(2),
        ..PaintSpec::default()
    };
    let painted = Painted::new(&history, Some("e"), &spec);

    let dimmed: Vec<bool> = painted
        .paint
        .node_style
        .iter()
        .map(|s| s & PAINT_DIM != 0)
        .collect();
    assert_eq!(dimmed, vec![false, false, false, true, false]);
    let side = painted.paint.node_lane[3];
    for (row, s, style, lane) in painted.segments() {
        assert_eq!(style & PAINT_DIM != 0, lane == side, "row {row}: {s:?}");
    }
}

#[test]
fn ancestry_of_a_row_past_the_end_dims_everything() {
    let history = nodes(&[("b", &["a"]), ("a", &[])]);
    let spec = PaintSpec {
        ancestry_of: Some(9),
        ..PaintSpec::default()
    };
    let painted = Painted::new(&history, Some("b"), &spec);
    assert!(painted.paint.node_style.iter().all(|s| s & PAINT_DIM != 0));
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
    for ((history, primary), long) in random_histories()
        .into_iter()
        .zip([0, 3].into_iter().cycle())
    {
        let painted = Painted::cut(&history, primary.as_deref(), long, &PaintSpec::default());
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
            // A stub takes the colour of its node, and the lane of the link it stands for.
            let index = painted.rows[r]
                .segments
                .iter()
                .position(|other| std::ptr::eq(other, s))
                .unwrap();
            if painted.stub(r, index) {
                continue;
            }
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
    for ((history, primary), long) in random_histories()
        .into_iter()
        .zip([0, 3].into_iter().cycle())
    {
        let painted = Painted::cut(&history, primary.as_deref(), long, &PaintSpec::default());
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

/// Both stubs of a cut link are one lane, so the branch they belong to stays one line.
#[test]
fn the_two_stubs_of_a_cut_link_share_its_lane_on_random_histories() {
    let mut stubs = 0;
    for (history, primary) in random_histories() {
        let painted = Painted::cut(&history, primary.as_deref(), 3, &PaintSpec::default());
        let row_of = |oid: &str| history.iter().position(|n| n.oid == oid).unwrap();
        let lane = |row: usize, index: u16| {
            painted.paint.segment_lane
                [painted.paint.segment_first[row] as usize + usize::from(index)]
        };
        for (p, row) in painted.rows.iter().enumerate() {
            for link in &row.links {
                let segment = &row.segments[usize::from(link.segment)];
                if segment.span != Span::Top {
                    continue;
                }
                stubs += 1;
                let children: Vec<u32> = row
                    .links
                    .iter()
                    .filter(|l| l.segment == link.segment)
                    .flat_map(|l| {
                        let c = row_of(&l.oid);
                        painted.rows[c]
                            .links
                            .iter()
                            .filter(|down| row_of(&down.oid) == p)
                            .map(move |down| lane(c, down.segment))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                assert!(
                    children.contains(&lane(p, link.segment)),
                    "row {p}: the stub above it has none of its children's lanes
{history:?}"
                );
            }
        }
    }
    assert!(stubs > 50, "the histories cut only {stubs} links");
}

#[test]
fn a_ticked_branch_keeps_its_colour_across_a_cut_link() {
    let history = nodes(&[
        ("m6", &["m5"]),
        ("b1", &["m0"]),
        ("m5", &["m4"]),
        ("m4", &["m3"]),
        ("m3", &["m2"]),
        ("m2", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::cut(&history, Some("m6"), 2, &tips(&[(1, 5)]));
    assert!(
        !painted.rows[1].links.is_empty(),
        "b1's link is cut: {:?}",
        painted.rows[1]
    );
    for (row, s, style, lane) in painted.segments() {
        if !s.primary {
            assert_eq!(style, 6, "row {row}: {s:?}");
            assert_eq!(lane, painted.paint.node_lane[1], "row {row}: {s:?}");
        }
    }
}

/// A cut first-parent link is still the branch's line (07 §5): the commit below the arrow
/// goes on in its child's lane, and in its colour.
#[test]
fn a_branch_goes_on_in_one_lane_below_a_cut_first_parent_link() {
    let history = nodes(&[
        ("m6", &["m5"]),
        ("c", &["p"]),
        ("m5", &["m4"]),
        ("m4", &["m3"]),
        ("m3", &["m2"]),
        ("p", &["m1"]),
        ("m2", &["m1"]),
        ("m1", &[]),
    ]);
    let painted = Painted::cut(&history, Some("m6"), 2, &PaintSpec::default());
    assert!(
        !painted.rows[1].links.is_empty(),
        "c's link to p is cut: {:?}",
        painted.rows[1]
    );

    assert_eq!(painted.paint.node_lane[5], painted.paint.node_lane[1]);
    assert_eq!(painted.rows[5].color, painted.rows[1].color);
}

fn mergeable(chosen: u32) -> PaintSpec {
    PaintSpec {
        mergeable_of: Some(chosen),
        ..PaintSpec::default()
    }
}

fn dimmed(painted: &Painted) -> Vec<bool> {
    painted
        .paint
        .node_style
        .iter()
        .map(|s| s & PAINT_DIM != 0)
        .collect()
}

/// Mergeable Coloring (SmartGit): what a merge of the chosen commit into the main line
/// would bring stands out, everything else is dimmed.
#[test]
fn mergeable_lights_what_a_merge_would_bring() {
    let history = nodes(&[
        ("m3", &["m2"]),
        ("f2", &["f1"]),
        ("m2", &["m1"]),
        ("f1", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    let painted = Painted::new(&history, Some("m3"), &mergeable(1));

    assert_eq!(dimmed(&painted), vec![true, false, true, false, true, true]);
    for (row, s, style, _) in painted.segments() {
        let lit = !s.primary && (row == 1 || row == 2 || (row == 3 && s.span == Span::Top));
        assert_eq!(style & PAINT_DIM == 0, lit, "row {row}: {s:?}");
    }
}

#[test]
fn mergeable_leaves_out_what_the_main_line_merged_already() {
    let history = nodes(&[
        ("f3", &["f2"]),
        ("m3", &["m2", "f2"]),
        ("f2", &["f1"]),
        ("m2", &["m1"]),
        ("f1", &["m1"]),
        ("m1", &[]),
    ]);
    let painted = Painted::new(&history, Some("m3"), &mergeable(0));

    assert_eq!(dimmed(&painted), vec![false, true, true, true, true, true]);
}

#[test]
fn mergeable_of_a_main_line_commit_dims_everything() {
    let history = nodes(&[("m2", &["m1"]), ("f1", &["m1"]), ("m1", &[])]);
    let painted = Painted::new(&history, Some("m2"), &mergeable(2));
    assert!(dimmed(&painted).iter().all(|dim| *dim));
}

#[test]
fn mergeable_outranks_ancestry() {
    let history = nodes(&[("m2", &["m1"]), ("f1", &["m1"]), ("m1", &[])]);
    let spec = PaintSpec {
        mergeable_of: Some(1),
        ancestry_of: Some(1),
        ..PaintSpec::default()
    };
    let painted = Painted::new(&history, Some("m2"), &spec);
    assert_eq!(dimmed(&painted), vec![true, false, true]);
}
