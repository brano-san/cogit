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

// After a commit the panels reopened the repository: status, registration and watcher
// included, and the status is re-read by the refresh that follows anyway (R-316).
#[test]
fn the_refs_after_a_commit_show_the_new_head_without_reopening() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    state.stage_paths(repo, &["fresh.txt".to_owned()]).unwrap();
    let oid = state
        .commit(
            repo,
            &git_engine::CommitRequest {
                message: "add fresh.txt".to_owned(),
                amend: false,
                no_verify: false,
                only: Vec::new(),
            },
        )
        .unwrap();
    let mut events = state.subscribe();
    state.clear_command_log();

    let refs = state.repo_refs(repo).unwrap();

    assert_eq!(
        refs.head,
        git_engine::Head::Branch {
            name: "main".to_owned(),
            oid: oid.clone()
        }
    );
    let main = refs
        .branches
        .iter()
        .find(|branch| branch.name == "main")
        .unwrap();
    assert_eq!(main.oid, oid);
    assert_eq!(refs.state, git_engine::RepoState::Clean);
    assert!(state.command_log().is_empty(), "{:?}", state.command_log());
    assert!(events.try_recv().is_err(), "a re-read is not an open");
}

#[test]
fn the_refs_of_an_unknown_repository_are_an_error() {
    assert!(AppState::new().repo_refs(RepoId(42)).is_err());
}
