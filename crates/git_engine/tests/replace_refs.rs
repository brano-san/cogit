// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! gix 0.87.1 reads `core.useReplaceRefs` upside down: with the key missing or true it
//! ignores `refs/replace/`, with false it applies them. Cogit pins "ignore" (R-161 note on
//! replaced history), so the graph shows the stored history whatever the user's config says.
//! When gix is fixed these fail: the pin, the health card and the risk entry go together.

use git_engine::{CommitQuery, RepoHandle};
use test_fixtures::Fixture;

/// `a`; an unrelated root `b` with `c` on it; `b` grafted onto `a` by a replacement.
fn grafted(graph: bool) -> (Fixture, String) {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "a.txt", "a\n").unwrap();
    let a = f.oid("HEAD").unwrap();
    f.git(&["switch", "-q", "--orphan", "side"]).unwrap();
    let b = f.commit_file(2, "b.txt", "b\n").unwrap();
    f.commit_file(3, "c.txt", "c\n").unwrap();
    if graph {
        f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    }
    f.git(&["replace", "--graft", &b, &a]).unwrap();
    (f, b)
}

fn parents_of_b(f: &Fixture, b: &str) -> Vec<String> {
    let repo = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        visible_refs: Some(vec!["HEAD".to_owned()]),
        ..CommitQuery::default()
    };
    let mut parents = None;
    repo.search_commits(&query, 100, |chunk| {
        parents = chunk
            .into_iter()
            .find(|row| row.oid == b)
            .map(|row| row.parents);
        true
    })
    .unwrap();
    parents.unwrap()
}

#[test]
fn the_graph_shows_the_stored_history_without_the_key() {
    let (f, b) = grafted(true);
    assert!(parents_of_b(&f, &b).is_empty());
    let stored = f
        .git(&["--no-replace-objects", "log", "--format=%H %P", "-1", &b])
        .unwrap();
    assert_eq!(stored.trim(), b);
}

#[test]
fn the_graph_shows_the_stored_history_when_the_key_is_true() {
    let (f, b) = grafted(true);
    f.git(&["config", "core.useReplaceRefs", "true"]).unwrap();
    assert!(parents_of_b(&f, &b).is_empty());
}

#[test]
fn the_graph_shows_the_stored_history_when_the_key_is_false() {
    let (f, b) = grafted(false);
    f.git(&["config", "core.useReplaceRefs", "false"]).unwrap();
    assert!(parents_of_b(&f, &b).is_empty());
}
