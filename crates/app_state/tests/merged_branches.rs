// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

/// `topic` was merged into `main` and deleted on `origin`; `keep` still has its upstream.
fn merged_and_cleaned_up() -> test_fixtures::Fixture {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["branch", "topic", "main~1"]).unwrap();
    f.git(&["branch", "keep", "main~1"]).unwrap();
    f.git(&["push", "--set-upstream", "origin", "topic", "keep"])
        .unwrap();
    f.git(&["push", "origin", "--delete", "topic"]).unwrap();
    f.git(&["fetch", "--prune", "origin"]).unwrap();
    f
}

#[test]
fn deleting_merged_branches_removes_only_the_gone_ones() {
    let f = merged_and_cleaned_up();
    let (state, repo) = open(&f);

    assert_eq!(state.delete_merged_branches(repo).unwrap(), ["topic"]);

    let left = f.git(&["branch", "--format=%(refname:short)"]).unwrap();
    assert!(!left.lines().any(|name| name == "topic"), "{left}");
    assert!(left.lines().any(|name| name == "keep"), "{left}");
}

#[test]
fn a_deleted_branch_can_be_brought_back_with_undo() {
    let f = merged_and_cleaned_up();
    let (state, repo) = open(&f);
    let tip = f.oid("topic").unwrap();

    state.delete_merged_branches(repo).unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("topic").unwrap(), tip);
}

#[test]
fn nothing_to_delete_is_an_empty_list_not_an_error() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = open(&f);
    assert!(state.delete_merged_branches(repo).unwrap().is_empty());
}

// F-311 end to end: the teammate deletes the merged branch on the server, and only Pull
// runs here — no `fetch --prune` of the fixture's own.
#[test]
fn a_branch_the_remote_deleted_goes_after_a_pull() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.git(&["branch", "topic", "main~1"]).unwrap();
    f.git(&["push", "--set-upstream", "origin", "topic"])
        .unwrap();
    let server = f.git(&["remote", "get-url", "origin"]).unwrap();
    f.git_in(
        std::path::Path::new(server.trim()),
        &["branch", "-D", "topic"],
    )
    .unwrap();
    let (state, repo) = open(&f);

    state.pull(repo, "origin", true, |_| {}).unwrap();

    assert_eq!(state.delete_merged_branches(repo).unwrap(), ["topic"]);
}
