// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The view decides which walked commits the graph shows before anything is laid out.

use graph_engine::ViewFilter;

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

#[test]
fn first_parent_keeps_one_line_and_drops_what_the_merge_brought_in() {
    let kept = run(
        &mut ViewFilter::first_parent(["m3".to_owned()]),
        MERGED,
        100,
    );
    assert_eq!(oids(&kept), ["m3", "m2", "m1", "m0"]);
    assert_eq!(kept[0].1, ["m2"], "the merge draws no line to the branch");
}

#[test]
fn first_parent_follows_every_ref_the_walk_started_from() {
    let roots = ["m3".to_owned(), "t2".to_owned()];
    let kept = run(&mut ViewFilter::first_parent(roots), MERGED, 100);
    assert_eq!(oids(&kept), ["m3", "t2", "m2", "t1", "m1", "m0"]);
    assert_eq!(kept[0].1, ["m2"]);
}

#[test]
fn first_parent_decides_the_same_in_chunks_as_all_at_once() {
    let whole = run(
        &mut ViewFilter::first_parent(["m3".to_owned()]),
        MERGED,
        100,
    );
    let chunked = run(&mut ViewFilter::first_parent(["m3".to_owned()]), MERGED, 1);
    assert_eq!(whole, chunked);
}

#[test]
fn first_parent_of_a_history_without_merges_keeps_everything() {
    let linear: History = &[("c", &["b"]), ("b", &["a"]), ("a", &[])];
    let kept = run(&mut ViewFilter::first_parent(["c".to_owned()]), linear, 100);
    assert_eq!(oids(&kept), ["c", "b", "a"]);
}
