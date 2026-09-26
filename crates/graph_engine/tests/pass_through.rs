// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A filtered graph drawn with lines (F-561): a commit the filter leaves out is passed
//! through, and a line waiting for it goes on to its first parent.

use graph_engine::{CommitNode, GraphRow, LayoutCursor, Span, pass_through, push};

fn node(oid: &str, parents: &[&str]) -> CommitNode {
    CommitNode {
        oid: oid.to_owned(),
        parents: parents.iter().map(|p| (*p).to_owned()).collect(),
        hidden: Vec::new(),
    }
}

fn into_node(row: &GraphRow) -> usize {
    row.segments
        .iter()
        .filter(|s| s.span == Span::Top && s.to == row.lane && !s.arrow)
        .count()
}

#[test]
fn a_match_joins_the_next_match_down_its_first_parents() {
    let mut cursor = LayoutCursor::with_mainline(Some("a".to_owned()));
    let mut rows = push(vec![node("a", &["b"])], &mut cursor);
    pass_through(&mut cursor, "b", Some("c"));
    rows.extend(push(vec![node("c", &[])], &mut cursor));

    assert_eq!(rows[1].lane, rows[0].lane);
    assert!(rows[1].primary, "the main line goes on through b");
    assert_eq!(into_node(&rows[1]), 1, "a's line ends in c: {:?}", rows[1]);
    assert!(rows[0].segments.iter().all(|s| !s.arrow));
}

#[test]
fn a_merged_line_goes_on_through_the_commits_left_out() {
    let mut cursor = LayoutCursor::with_mainline(Some("m".to_owned()));
    let mut rows = push(vec![node("m", &["x", "s"])], &mut cursor);
    pass_through(&mut cursor, "s", Some("t"));
    rows.extend(push(vec![node("t", &["x"]), node("x", &[])], &mut cursor));

    assert_eq!(into_node(&rows[1]), 1, "the merge's second line ends in t");
    assert_eq!(into_node(&rows[2]), 2, "m's and t's lines end in x");
}

#[test]
fn a_commit_left_out_with_no_parent_leaves_the_line_waiting() {
    let mut cursor = LayoutCursor::with_mainline(Some("a".to_owned()));
    let mut rows = push(vec![node("a", &["root"])], &mut cursor);
    pass_through(&mut cursor, "root", None);
    rows.extend(push(vec![node("other", &[])], &mut cursor));

    assert!(
        rows[1]
            .segments
            .iter()
            .any(|s| s.span == Span::Through && s.from == 0),
        "the history goes on below the list: {:?}",
        rows[1]
    );
}
