// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A history of many branches open at once, drawn whole (`long_links = 0`).

use graph_engine::{CommitNode, LayoutCursor, NodeKind, Segment, Span, finish, push};

const BRANCHES: usize = 300;
const DEPTH: usize = 40;

/// `BRANCHES` unrelated lines of `DEPTH` commits, interleaved row by row, so every line is
/// open from the first rows to the last.
fn interleaved() -> Vec<CommitNode> {
    let mut nodes = Vec::with_capacity(BRANCHES * DEPTH);
    for depth in 0..DEPTH {
        for branch in 0..BRANCHES {
            nodes.push(CommitNode {
                oid: format!("b{branch}c{depth}"),
                parents: if depth + 1 < DEPTH {
                    vec![format!("b{branch}c{}", depth + 1)]
                } else {
                    Vec::new()
                },
                hidden: Vec::new(),
            });
        }
    }
    nodes
}

#[test]
fn many_open_lines_are_laid_out_as_they_always_were() {
    let mut cursor = LayoutCursor::default();
    let mut rows = push(interleaved(), &mut cursor);
    rows.extend(finish(&mut cursor));

    assert_eq!(rows.len(), BRANCHES * DEPTH);

    // Written out, not hashed: line `b` keeps column `b` while every other line passes
    // straight through, and a line's own column carries what arrives and what leaves.
    let segment = |lane: usize, span: Span| Segment {
        from: u16::try_from(lane).unwrap(),
        to: u16::try_from(lane).unwrap(),
        span,
        primary: false,
        color: u8::try_from(lane % 8).unwrap(),
        arrow: false,
    };
    for depth in 0..DEPTH - 1 {
        for branch in 0..BRANCHES {
            let row = &rows[depth * BRANCHES + branch];
            let open = if depth == 0 { branch + 1 } else { BRANCHES };
            let mut expected = Vec::new();
            // What leaves a node is listed after everything that was already there.
            let leaving = segment(branch, Span::Bottom);
            for lane in 0..open {
                if lane != branch {
                    expected.push(segment(lane, Span::Through));
                    continue;
                }
                if depth > 0 {
                    expected.push(segment(lane, Span::Top));
                }
            }
            expected.push(leaving);
            assert_eq!(row.lane, u16::try_from(branch).unwrap(), "row {}", row.row);
            assert_eq!(usize::from(row.width), open, "row {}", row.row);
            assert_eq!(row.kind, NodeKind::Normal, "row {}", row.row);
            assert_eq!(row.segments, expected, "row {}", row.row);
            assert!(row.links.is_empty());
        }
    }
    // The last commits are roots: each line ends at its node and the columns close up
    // onto column 0.
    for branch in 0..BRANCHES {
        let row = &rows[(DEPTH - 1) * BRANCHES + branch];
        assert_eq!(row.kind, NodeKind::Root, "row {}", row.row);
        assert_eq!(row.lane, 0, "row {}", row.row);
    }
    assert_eq!(rows[(DEPTH - 1) * BRANCHES].segments.len(), BRANCHES);
}
