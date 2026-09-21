//! The parser's part of the three-way merge (doc/08-diff-engine.md §8): which top-level
//! node of the base each edit belongs to, and whether two sides can therefore be combined.

use crate::merge::{Edit, apply};
use std::ops::Range;

pub(crate) type Node = Range<usize>;

pub(crate) fn top_level_nodes(base: &str, language: &str) -> Option<Vec<Node>> {
    let grammar = match language {
        "cpp" | "c++" | "c" => tree_sitter_cpp::LANGUAGE.into(),
        "typescript" | "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => return None,
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar).ok()?;
    let tree = parser.parse(base, None)?;
    let root = tree.root_node();
    // A tree with an error in it says nothing trustworthy about where a node ends, and
    // the whole point of this step is certainty.
    if root.has_error() {
        return None;
    }

    let mut cursor = root.walk();
    Some(
        root.named_children(&mut cursor)
            .map(|node| node.start_position().row..node.end_position().row + 1)
            .collect(),
    )
}

fn node_of(nodes: &[Node], edit: &Edit) -> Option<usize> {
    // An insertion has an empty base range; it belongs to the node it is inside.
    let touched = edit.base.start..edit.base.end.max(edit.base.start + 1);
    let mut found = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.start < touched.end && touched.start < node.end {
            if found.is_some() {
                return None;
            }
            found = Some(index);
        }
    }
    found
}

/// The two sides combined, when every edit of each lands in a node the other never
/// touched. Anything less certain returns `None` and the region stays a conflict.
pub(crate) fn settle(
    nodes: &[Node],
    base: &[String],
    span: Range<usize>,
    ours: &[Edit],
    theirs: &[Edit],
) -> Option<Vec<String>> {
    if ours.is_empty() || theirs.is_empty() {
        return None;
    }

    let mine: Vec<usize> = ours
        .iter()
        .map(|edit| node_of(nodes, edit))
        .collect::<Option<_>>()?;
    let yours: Vec<usize> = theirs
        .iter()
        .map(|edit| node_of(nodes, edit))
        .collect::<Option<_>>()?;
    if mine.iter().any(|node| yours.contains(node)) {
        return None;
    }

    let mut combined: Vec<&Edit> = ours.iter().chain(theirs.iter()).collect();
    combined.sort_by_key(|edit| edit.base.start);
    // Two edits that overlap in the base cannot both be applied, whatever the tree says.
    if combined
        .windows(2)
        .any(|pair| pair[0].base.end > pair[1].base.start)
    {
        return None;
    }

    let owned: Vec<Edit> = combined
        .into_iter()
        .map(|edit| Edit {
            base: edit.base.clone(),
            lines: edit.lines.clone(),
        })
        .collect();
    Some(apply(base, &owned, span))
}
