// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};

#[test]
fn details_resolve_through_the_registry() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let details = state.commit_details(repo, "HEAD").unwrap();

    assert_eq!(details.summary, "commit 2");
    assert_eq!(details.oid, f.oid("HEAD").unwrap());
}

#[test]
fn files_resolve_through_the_registry() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let files = state.commit_files(repo, "HEAD").unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "file2.txt");
}

#[test]
fn an_unknown_repo_id_is_an_error() {
    let state = AppState::new();

    assert!(state.commit_details(RepoId(42), "HEAD").is_err());
    assert!(state.commit_files(RepoId(42), "HEAD").is_err());
}
