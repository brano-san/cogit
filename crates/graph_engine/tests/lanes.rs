// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Lane allocation for the commit graph.
//!
//! Snapshots are ASCII drawings rather than dumps of the structures: a regression in
//! graph layout has to be visible to a human reading the diff, which a list of numbers
//! is not (`doc/09-testing.md` section 5).

use graph_engine::{CommitNode, GraphLayout, LayoutCursor, NodeKind, layout};

/// Builds commits from a compact description: `("a", &["b", "c"])` is commit `a` with
/// parents `b` and `c`. Order is the topological order the graph will be drawn in.
fn commits(spec: &[(&str, &[&str])]) -> Vec<CommitNode> {
    spec.iter()
        .map(|(oid, parents)| CommitNode {
            oid: (*oid).to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
        })
        .collect()
}

/// Draws the layout the way `git log --graph` would, so a snapshot diff is readable.
fn render(nodes: &[CommitNode], out: &GraphLayout) -> String {
    let width = usize::from(out.max_lane) + 1;
    let mut text = String::new();

    for (row, node) in nodes.iter().enumerate() {
        let row_u32 = u32::try_from(row).unwrap();
        let placement = out.lanes.iter().find(|l| l.row == row_u32).unwrap();

        // Everything drawn at this row: the node itself plus any line arriving here.
        let mut occupied = vec![false; width];
        occupied[usize::from(placement.lane)] = true;
        for edge in out.edges.iter().filter(|e| e.to_row == row_u32) {
            occupied[usize::from(edge.to_lane)] = true;
        }
        for edge in out.edges.iter().filter(|e| e.from_row == row_u32) {
            occupied[usize::from(edge.from_lane)] = true;
        }

        let mut line: Vec<char> = Vec::with_capacity(width * 2);
        for (lane, &busy) in occupied.iter().enumerate() {
            line.push(if lane == usize::from(placement.lane) {
                match placement.kind {
                    NodeKind::Merge => '*',
                    NodeKind::Root => 'o',
                    _ => '●',
                }
            } else if busy {
                '|'
            } else {
                ' '
            });
            line.push(' ');
        }
        text.push_str(&format!(
            "{}  {}  lane={} colour={}\n",
            line.iter().collect::<String>().trim_end(),
            node.oid,
            placement.lane,
            placement.color
        ));

        // The band below this row, showing which lines change lane.
        let band: Vec<String> = out
            .edges
            .iter()
            .filter(|e| e.from_row == row_u32)
            .map(|e| format!("{}->{}", e.from_lane, e.to_lane))
            .collect();
        if !band.is_empty() {
            text.push_str(&format!(
                "{:width$}  [{}]\n",
                "",
                band.join(" "),
                width = width * 2
            ));
        }
    }
    text
}

fn run(spec: &[(&str, &[&str])]) -> (Vec<CommitNode>, GraphLayout) {
    let nodes = commits(spec);
    let mut cursor = LayoutCursor::default();
    let out = layout(&nodes, &mut cursor);
    (nodes, out)
}

#[test]
fn linear_history_stays_in_a_single_lane() {
    let (_, out) = run(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
    assert!(out.lanes.iter().all(|l| l.lane == 0));
    assert_eq!(out.max_lane, 0);
}

#[test]
fn the_first_parent_keeps_the_lane_of_its_child() {
    // Keeps mainline history vertical instead of zig-zagging across lanes.
    let (_, out) = run(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    let merge_lane = out.lanes[0].lane;
    let first_parent_lane = out.lanes[1].lane;
    assert_eq!(merge_lane, first_parent_lane);
}

#[test]
fn a_root_commit_is_marked_as_such() {
    let (_, out) = run(&[("b", &["a"]), ("a", &[])]);
    assert_eq!(out.lanes[1].kind, NodeKind::Root);
}

#[test]
fn a_commit_with_two_parents_is_marked_as_a_merge() {
    let (_, out) = run(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    assert_eq!(out.lanes[0].kind, NodeKind::Merge);
}

#[test]
fn a_diamond_widens_to_two_lanes_and_comes_back() {
    let (_, out) = run(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    assert_eq!(out.max_lane, 1, "a diamond needs exactly two lanes");
    // The shared root returns to the mainline lane.
    assert_eq!(out.lanes[3].lane, 0);
}

#[test]
fn an_octopus_merge_gathers_three_parents() {
    let (_, out) = run(&[
        ("m", &["a", "b", "c"]),
        ("a", &["r"]),
        ("b", &["r"]),
        ("c", &["r"]),
        ("r", &[]),
    ]);
    assert_eq!(out.lanes[0].kind, NodeKind::Merge);
    // Three lines leave the merge downwards.
    let leaving = out.edges.iter().filter(|e| e.from_row == 0).count();
    assert_eq!(leaving, 3);
}

#[test]
fn independent_roots_are_never_joined() {
    // They may well share a lane once the first one ends — `git log --graph` does the
    // same. What must not happen is an edge implying one descends from the other.
    let (_, out) = run(&[("a", &[]), ("b", &[])]);
    assert!(
        out.edges.is_empty(),
        "unrelated roots must not be connected"
    );
    assert!(out.lanes.iter().all(|l| l.kind == NodeKind::Root));
}

#[test]
fn a_lane_is_reused_once_its_branch_has_ended() {
    // Without reuse the graph creeps rightwards forever on a busy repository.
    let (_, out) = run(&[
        ("m", &["a", "b"]),
        ("a", &["r"]),
        ("b", &["r"]),
        ("r", &[]),
        ("x", &[]),
    ]);
    assert!(
        out.max_lane <= 1,
        "lane 1 should be reused, got max_lane {}",
        out.max_lane
    );
}

#[test]
fn colours_survive_the_chunk_boundary() {
    // The graph streams in chunks; a branch that changes colour mid-scroll is a bug
    // the user sees immediately (doc/07-graph-rendering.md).
    let all = commits(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    let mut whole = LayoutCursor::default();
    let reference = layout(&all, &mut whole);

    let mut streamed = LayoutCursor::default();
    let first = layout(&all[..2], &mut streamed);
    let second = layout(&all[2..], &mut streamed);

    let joined: Vec<u8> = first
        .lanes
        .iter()
        .chain(second.lanes.iter())
        .map(|l| l.color)
        .collect();
    let expected: Vec<u8> = reference.lanes.iter().map(|l| l.color).collect();
    assert_eq!(joined, expected);
}

#[test]
fn rows_continue_counting_across_chunks() {
    let all = commits(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
    let mut cursor = LayoutCursor::default();
    let first = layout(&all[..1], &mut cursor);
    let second = layout(&all[1..], &mut cursor);
    assert_eq!(first.lanes[0].row, 0);
    assert_eq!(second.lanes[0].row, 1);
    assert_eq!(second.lanes[1].row, 2);
}

#[test]
fn an_empty_chunk_produces_an_empty_layout() {
    let mut cursor = LayoutCursor::default();
    let out = layout(&[], &mut cursor);
    assert!(out.lanes.is_empty());
    assert!(out.edges.is_empty());
}

// --- snapshots ---------------------------------------------------------------

#[test]
fn snapshot_diamond() {
    let (nodes, out) = run(&[("m", &["a", "b"]), ("a", &["r"]), ("b", &["r"]), ("r", &[])]);
    insta::assert_snapshot!(render(&nodes, &out));
}

#[test]
fn snapshot_octopus() {
    let (nodes, out) = run(&[
        ("m", &["a", "b", "c"]),
        ("a", &["r"]),
        ("b", &["r"]),
        ("c", &["r"]),
        ("r", &[]),
    ]);
    insta::assert_snapshot!(render(&nodes, &out));
}

#[test]
fn snapshot_two_roots() {
    let (nodes, out) = run(&[("b", &["a"]), ("a", &[]), ("y", &["x"]), ("x", &[])]);
    insta::assert_snapshot!(render(&nodes, &out));
}

#[test]
fn snapshot_nested_branches() {
    // Several branches open and close, exercising lane reuse and crossings.
    let (nodes, out) = run(&[
        ("h", &["g", "f"]),
        ("g", &["e"]),
        ("f", &["d"]),
        ("e", &["c"]),
        ("d", &["c"]),
        ("c", &["b"]),
        ("b", &["a"]),
        ("a", &[]),
    ]);
    insta::assert_snapshot!(render(&nodes, &out));
}
