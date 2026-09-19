// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Tags, as shown on the ref capsules of the commit graph.

use git_engine::RepoHandle;
use test_fixtures::Fixture;

fn tagged() -> Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["tag", "v1.0.0"]).unwrap();
    f.git_at(20, &["tag", "-a", "v2.0.0", "-m", "release two"])
        .unwrap();
    f.git(&["tag", "old", "HEAD~2"]).unwrap();
    f
}

#[test]
fn lists_every_tag() {
    // The fixture must be bound: a temporary would delete its directory at the end of
    // this statement, leaving the handle pointing at nothing.
    let f = tagged();
    let repo = RepoHandle::open(f.path()).unwrap();
    let names: Vec<String> = repo.tags().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(
        names,
        vec!["old", "v1.0.0", "v2.0.0"],
        "tags come back sorted"
    );
}

#[test]
fn a_lightweight_tag_points_straight_at_its_commit() {
    let f = tagged();
    let repo = RepoHandle::open(f.path()).unwrap();
    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "v1.0.0")
        .unwrap();

    assert!(!tag.is_annotated);
    assert_eq!(tag.oid, f.oid("HEAD").unwrap());
}

#[test]
fn an_annotated_tag_resolves_to_the_commit_it_marks() {
    // The ref points at a tag object; the graph needs the commit behind it, otherwise
    // the capsule would attach to no row at all.
    let f = tagged();
    let repo = RepoHandle::open(f.path()).unwrap();
    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "v2.0.0")
        .unwrap();

    assert!(tag.is_annotated);
    assert_eq!(tag.oid, f.oid("HEAD").unwrap());
}

#[test]
fn a_tag_on_an_older_commit_keeps_its_own_target() {
    let f = tagged();
    let repo = RepoHandle::open(f.path()).unwrap();
    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "old")
        .unwrap();

    assert_eq!(tag.oid, f.oid("HEAD~2").unwrap());
}

#[test]
fn tags_carry_their_full_ref_name() {
    let f = tagged();
    let repo = RepoHandle::open(f.path()).unwrap();
    let tag = repo.tags().unwrap().into_iter().next().unwrap();
    assert!(tag.full_name.starts_with("refs/tags/"));
}

#[test]
fn a_repository_without_tags_returns_an_empty_list() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(repo.tags().unwrap().is_empty());
}
