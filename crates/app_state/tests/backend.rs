#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, Host};
use git_engine::WorktreeView;
use serde::Serialize;
use serde_json::Value;

fn loopback() -> Host {
    Host::Loopback(env!("CARGO_BIN_EXE_cogit-agent").into())
}

fn json(value: &impl Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

fn without_id(summary: &app_state::RepoSummary) -> Value {
    let mut value = json(summary);
    value.as_object_mut().unwrap().remove("repo");
    value
}

fn changed() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("file0.txt", "edited\n").unwrap();
    f.write_file("new.txt", "new\n").unwrap();
    f.git(&["add", "new.txt"]).unwrap();
    f
}

#[test]
fn a_repository_opened_through_the_agent_reads_as_the_local_one() {
    let f = changed();
    let (local, remote) = (AppState::new(), AppState::new());
    let near = local.open_repository(f.path()).unwrap();
    let far = remote.open_repository_on(&loopback(), f.path()).unwrap();

    assert!(remote.is_remote(far.repo));
    assert!(!local.is_remote(near.repo));
    assert_eq!(without_id(&far), without_id(&near));
    assert_eq!(
        json(&remote.repo_status(far.repo).unwrap()),
        json(&local.repo_status(near.repo).unwrap())
    );
    assert_eq!(
        json(
            &remote
                .worktree_files(far.repo, WorktreeView::default())
                .unwrap()
        ),
        json(
            &local
                .worktree_files(near.repo, WorktreeView::default())
                .unwrap()
        )
    );
    assert_eq!(
        json(&remote.repo_refs(far.repo).unwrap()),
        json(&local.repo_refs(near.repo).unwrap())
    );
}

#[test]
fn an_error_in_the_agent_arrives_as_the_same_git_error() {
    let folder = test_fixtures::tempdir().unwrap();
    let near = AppState::new().open_repository(folder.path()).unwrap_err();
    let far = AppState::new()
        .open_repository_on(&loopback(), folder.path())
        .unwrap_err();
    assert_eq!(json(&far), json(&near));
}

#[test]
fn local_and_agent_repositories_keep_their_own_ids() {
    let (one, two, three) = (
        test_fixtures::linear(1).unwrap(),
        changed(),
        test_fixtures::linear(2).unwrap(),
    );
    let state = AppState::new();
    let host = loopback();
    let near = state.open_repository(one.path()).unwrap();
    let first = state.open_repository_on(&host, two.path()).unwrap();
    let second = state.open_repository_on(&host, three.path()).unwrap();

    let ids = [near.repo, first.repo, second.repo];
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::HashSet<_>>().len()
    );
    assert_eq!(state.repo_status(near.repo).unwrap().unstaged, 0);
    assert_eq!(state.repo_status(first.repo).unwrap().unstaged, 1);
    assert_eq!(state.repo_status(second.repo).unwrap().unstaged, 0);
    assert_eq!(state.get(first.repo).unwrap().root, dunce_root(two.path()));
}

#[test]
fn a_closed_agent_repository_answers_like_a_closed_local_one() {
    let f = changed();
    let state = AppState::new();
    let far = state.open_repository_on(&loopback(), f.path()).unwrap();
    assert!(state.close_repository(far.repo));
    assert!(!state.is_remote(far.repo));

    let near = AppState::new();
    let gone = near.open_repository(f.path()).unwrap();
    near.close_repository(gone.repo);
    assert_eq!(
        json(&state.repo_status(far.repo).unwrap_err()),
        json(&near.repo_status(gone.repo).unwrap_err())
    );
}

fn dunce_root(path: &std::path::Path) -> std::path::PathBuf {
    let state = AppState::new();
    let repo = state.open_repository(path).unwrap().repo;
    state.get(repo).unwrap().root
}
