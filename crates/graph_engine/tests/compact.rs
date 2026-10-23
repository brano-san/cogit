// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The compact layout of item 9 (07 §3): lines sit side by side, move left in a gap as
//! soon as a column frees, short lines take the near columns and long ones step aside, and
//! lines to one commit become one as early as possible. Each case pins every node's column
//! and the widest row.

use graph_engine::{CommitNode, GraphRow, LayoutCursor, finish, push};

fn run(spec: &[(&str, &[&str])]) -> Vec<GraphRow> {
    let nodes = spec
        .iter()
        .map(|(oid, parents)| CommitNode {
            oid: (*oid).to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            hidden: Vec::new(),
        })
        .collect();
    let mut cursor = LayoutCursor::with_mainline(Some(spec[0].0.to_owned())).with_lookahead(40);
    let mut rows = push(nodes, &mut cursor);
    rows.extend(finish(&mut cursor));
    rows
}

fn columns(rows: &[GraphRow]) -> Vec<u16> {
    rows.iter().map(|row| row.lane).collect()
}

fn widths(rows: &[GraphRow]) -> Vec<u16> {
    rows.iter().map(|row| row.width).collect()
}

/// A: one-commit branches off the main line and a long branch beside them. The short ones
/// take column 1; the long one steps to column 2 for their rows and comes back.
#[test]
fn short_branches_take_the_near_column_and_the_long_one_steps_aside() {
    let rows = run(&[
        ("m9", &["m8"]),
        ("long", &["m1"]),
        ("m8", &["m7"]),
        ("s1", &["m7"]),
        ("m7", &["m6"]),
        ("m6", &["m5"]),
        ("s2", &["m5"]),
        ("m5", &["m4"]),
        ("m4", &["m3"]),
        ("s3", &["m3"]),
        ("m3", &["m2"]),
        ("m2", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    assert_eq!(columns(&rows), [0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(widths(&rows), [1, 2, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 1, 1]);
    assert_eq!(rows.iter().map(|row| row.width).max(), Some(3));
}

/// B: two branches off one commit. The later one joins the earlier one's line under its own
/// node, and that line joins the main line under the main line's commit before the parent.
#[test]
fn two_children_of_one_commit_share_a_trunk_from_the_lower_child_down() {
    let rows = run(&[
        ("m3", &["m2"]),
        ("x", &["m1"]),
        ("y", &["m1"]),
        ("m2", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ]);
    assert_eq!(columns(&rows), [0, 1, 2, 0, 0, 0]);
    assert_eq!(widths(&rows), [1, 2, 3, 2, 1, 1]);
}

/// C: a merge whose second parent is the first parent's parent. Its line joins the first
/// parent's line under the first parent, not at the ring of the commit both wait for.
#[test]
fn a_second_parent_shared_with_the_first_parent_line_joins_it_early() {
    let rows = run(&[("m", &["a", "p"]), ("a", &["p"]), ("p", &["r"]), ("r", &[])]);
    assert_eq!(columns(&rows), [0, 0, 0, 0]);
    assert_eq!(widths(&rows), [2, 2, 1, 1]);
}
