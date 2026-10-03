// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A history of many branches open at once, drawn whole (`long_links = 0`).

use graph_engine::{CommitNode, LayoutCursor, finish, push};
use std::hash::{Hash as _, Hasher as _};

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
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{rows:?}").hash(&mut hasher);
    // Taken from the layout before the lane lookups went linear: same rows, 322 ms -> 35 ms.
    assert_eq!(hasher.finish(), 8_411_155_274_299_977_459);
}
