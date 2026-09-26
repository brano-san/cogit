// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The view decides which walked commits the graph shows before anything is laid out.

use graph_engine::{Fold, ViewFilter};

type History<'a> = &'a [(&'a str, &'a [&'a str])];

/// What stays, each with the parents it keeps, fed in chunks of `chunk`.
fn run(view: &mut ViewFilter, history: History, chunk: usize) -> Vec<(String, Vec<String>)> {
    let mut kept = Vec::new();
    for part in history.chunks(chunk) {
        for (oid, parents) in part {
            let mut parents: Vec<String> = parents.iter().map(|p| (*p).to_owned()).collect();
            if view.admit(oid, &mut parents) {
                kept.push(((*oid).to_owned(), parents));
            }
        }
    }
    kept
}

fn oids(kept: &[(String, Vec<String>)]) -> Vec<&str> {
    kept.iter().map(|(oid, _)| oid.as_str()).collect()
}

const MERGED: History = &[
    ("m3", &["m2", "t2"]),
    ("t2", &["t1"]),
    ("m2", &["m1"]),
    ("t1", &["m1"]),
    ("m1", &["m0"]),
    ("m0", &[]),
];

fn collapse(roots: &[&str], expanded: &[&str]) -> ViewFilter {
    ViewFilter::collapse_merged(
        roots.iter().map(|r| (*r).to_owned()),
        expanded.iter().map(|e| (*e).to_owned()),
    )
}

#[test]
fn a_merged_branch_folds_into_its_merge_with_a_count() {
    let mut view = collapse(&["m3"], &[]);
    let kept = run(&mut view, MERGED, 100);
    assert_eq!(oids(&kept), ["m3", "m2", "m1", "m0"]);
    assert_eq!(kept[0].1, ["m2"]);
    assert_eq!(view.take_folds(), [Fold { row: 0, hidden: 2 }]);
    assert_eq!(view.take_folds(), [], "reported once, until it grows again");
}

#[test]
fn an_expanded_merge_shows_its_branch() {
    let mut view = collapse(&["m3"], &["m3"]);
    let kept = run(&mut view, MERGED, 100);
    assert_eq!(kept.len(), MERGED.len());
    assert_eq!(kept[0].1, ["m2", "t2"]);
    assert_eq!(view.take_folds(), []);
}

#[test]
fn a_ticked_branch_the_merge_brought_in_keeps_its_line() {
    let mut view = collapse(&["m3", "t2"], &[]);
    let kept = run(&mut view, MERGED, 100);
    assert_eq!(kept.len(), MERGED.len());
    assert_eq!(
        kept[0].1,
        ["m2", "t2"],
        "the merge still draws its line to it"
    );
    assert_eq!(view.take_folds(), []);
}

#[test]
fn a_branch_merged_into_a_merged_branch_folds_with_it() {
    let nested: History = &[
        ("m2", &["m1", "t3"]),
        ("t3", &["t2", "u1"]),
        ("u1", &["t1"]),
        ("t2", &["t1"]),
        ("t1", &["m1"]),
        ("m1", &["m0"]),
        ("m0", &[]),
    ];
    let mut view = collapse(&["m2"], &[]);
    let kept = run(&mut view, nested, 2);
    assert_eq!(oids(&kept), ["m2", "m1", "m0"]);
    assert_eq!(view.take_folds(), [Fold { row: 0, hidden: 4 }]);
}
