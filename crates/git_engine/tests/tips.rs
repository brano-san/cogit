// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The graph is the union of history reachable from the ticked refs. Every start point is
//! peeled to a commit first; a ref that cannot be is left out and named, and never takes
//! the rest of the graph down with it (doc/12-risks.md, R-157).

use git_engine::{CommitQuery, RepoHandle};
use std::collections::HashSet;

fn walk(f: &test_fixtures::Fixture, refs: &[&str]) -> (Vec<String>, Vec<String>) {
    let handle = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        visible_refs: Some(refs.iter().map(|name| (*name).to_owned()).collect()),
        ..CommitQuery::default()
    };
    let mut oids = Vec::new();
    let skipped = handle
        .search_commits(&query, 50, |chunk| {
            oids.extend(chunk.into_iter().map(|row| row.oid));
            true
        })
        .unwrap();
    (oids, skipped.into_iter().map(|skip| skip.name).collect())
}

/// A commit only an annotated tag reaches: the case that used to crash the graph.
fn tagged_side_commit(f: &test_fixtures::Fixture) -> String {
    f.git(&["checkout", "-q", "-b", "side"]).unwrap();
    let side = f.commit_file(10, "side.txt", "side\n").unwrap();
    f.git(&["tag", "-a", "v-side", "-m", "side release"])
        .unwrap();
    f.git(&["checkout", "-q", "main"]).unwrap();
    f.git(&["branch", "-q", "-D", "side"]).unwrap();
    side
}

#[test]
fn an_annotated_tag_starts_the_walk_from_its_commit() {
    let f = test_fixtures::linear(2).unwrap();
    let side = tagged_side_commit(&f);

    let (oids, skipped) = walk(&f, &["HEAD", "refs/tags/v-side"]);

    assert!(oids.contains(&side), "the tagged commit is drawn");
    assert!(skipped.is_empty());
}

#[test]
fn a_tag_of_a_tag_is_peeled_all_the_way_to_the_commit() {
    let f = test_fixtures::linear(2).unwrap();
    let side = tagged_side_commit(&f);
    f.git(&["tag", "-a", "v-outer", "-m", "a tag of a tag", "v-side"])
        .unwrap();

    let (oids, skipped) = walk(&f, &["refs/tags/v-outer"]);

    assert!(oids.contains(&side));
    assert!(skipped.is_empty());
}

#[test]
fn a_tag_on_a_tree_is_left_out_and_named_while_the_rest_is_drawn() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "-a", "v-tree", "-m", "a tree", "HEAD^{tree}"])
        .unwrap();

    let (oids, skipped) = walk(&f, &["HEAD", "refs/tags/v-tree"]);

    assert_eq!(oids.len(), 2, "HEAD's history is still there");
    assert_eq!(skipped, vec!["refs/tags/v-tree".to_owned()]);
}

#[test]
fn a_ref_deleted_while_it_was_ticked_drops_out_without_a_word() {
    let f = test_fixtures::linear(2).unwrap();
    let (oids, skipped) = walk(&f, &["HEAD", "refs/heads/gone"]);
    assert_eq!(oids.len(), 2);
    assert!(skipped.is_empty());
}

#[test]
fn a_commit_reachable_from_two_ticked_refs_is_drawn_once() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["branch", "other", "HEAD~1"]).unwrap();

    let (oids, _) = walk(&f, &["HEAD", "refs/heads/main", "refs/heads/other"]);

    let unique: HashSet<&String> = oids.iter().collect();
    assert_eq!(unique.len(), oids.len());
    assert_eq!(oids.len(), 3);
}

#[test]
fn ticking_a_ref_whose_history_is_already_drawn_changes_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["tag", "-a", "v1", "-m", "v1", "HEAD~1"]).unwrap();

    let (before, _) = walk(&f, &["HEAD"]);
    let (after, _) = walk(&f, &["HEAD", "refs/tags/v1"]);

    assert_eq!(before, after);
}

#[test]
fn an_annotated_tag_labels_its_commit_not_the_tag_object() {
    let f = test_fixtures::linear(2).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).unwrap();
    f.git(&["tag", "-a", "v1", "-m", "v1"]).unwrap();
    let object = f.git(&["rev-parse", "refs/tags/v1"]).unwrap();
    assert_ne!(
        object.trim(),
        head.trim(),
        "the fixture made an annotated tag"
    );

    let tags = RepoHandle::open(f.path()).unwrap().tags().unwrap();

    assert_eq!(tags[0].oid, head.trim());
    assert!(tags[0].points_to_commit);
}

#[test]
fn a_tag_on_a_tree_says_so_for_its_checkbox() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["tag", "-a", "v-tree", "-m", "a tree", "HEAD^{tree}"])
        .unwrap();

    let tags = RepoHandle::open(f.path()).unwrap().tags().unwrap();

    assert!(!tags[0].points_to_commit);
}

/// A filtered list draws lines only between commits it shows; for the rest it needs to
/// know, as each match streams by, whether its parent will be in the list too (R-161).
#[test]
fn a_parent_is_shown_by_a_filter_only_if_it_matches_it() {
    let f = test_fixtures::linear(4).unwrap();
    let handle = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        message: Some("commit 2".to_owned()),
        ..CommitQuery::default()
    };
    let second = f.git(&["rev-parse", "HEAD~1"]).unwrap();
    let third = f.git(&["rev-parse", "HEAD"]).unwrap();

    assert!(handle.shown_by(&query, second.trim()));
    assert!(!handle.shown_by(&query, third.trim()));
    assert!(!handle.shown_by(&query, "not-an-oid"));
}
