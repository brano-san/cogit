// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;

fn open(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn calls_on_an_open_repository_share_one_opening() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);

    for _ in 0..5 {
        state.remotes(repo).unwrap();
    }

    assert_eq!(state.repositories_opened(), 1);
}

// The previous iteration read the flow config from before `flow_init` in the same handle.
#[test]
fn a_config_our_own_command_wrote_is_read_back() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    assert!(!state.flow_status(repo).unwrap().initialised);

    state
        .flow_init(repo, &git_engine::FlowConfig::default())
        .unwrap();

    assert!(state.flow_status(repo).unwrap().initialised);
}

#[test]
fn a_config_changed_from_a_terminal_is_read_back() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    assert!(state.remotes(repo).unwrap().is_empty());

    f.git(&["remote", "add", "upstream", "https://example.invalid/x.git"])
        .unwrap();

    assert_eq!(state.remotes(repo).unwrap(), vec!["upstream".to_owned()]);
    assert_eq!(state.repositories_opened(), 2);
}

#[test]
fn refs_moved_from_a_terminal_need_no_reopening() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    state.remotes(repo).unwrap();

    f.commit_file(30, "later.txt", "later\n").unwrap();
    f.git(&["branch", "topic"]).unwrap();

    let files = state.tree_files(repo, "topic").unwrap();
    assert!(files.iter().any(|name| name == "later.txt"), "{files:?}");
    assert_eq!(state.repositories_opened(), 1);
}

#[test]
fn closing_lets_go_of_the_opened_repository() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    state.remotes(repo).unwrap();
    assert_eq!(state.repositories_held(), 1);

    state.close_repository(repo);

    assert_eq!(state.repositories_held(), 0);
    assert!(state.remotes(repo).is_err());
}

#[test]
fn a_repository_deleted_under_us_is_an_error_not_a_stale_answer() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    state.remotes(repo).unwrap();

    std::fs::remove_dir_all(f.path().join(".git")).unwrap();

    assert!(state.remotes(repo).is_err());
    assert_eq!(state.repositories_held(), 0);
}
