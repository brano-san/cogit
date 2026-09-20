// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;

#[test]
fn several_repositories_can_be_open_at_once() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::branched().unwrap();
    let state = AppState::new();

    let first = state.open_repository(a.path()).unwrap().repo;
    let second = state.open_repository(b.path()).unwrap().repo;

    assert_ne!(first, second);
    assert_eq!(state.overviews().len(), 2);
}

#[test]
fn opening_the_same_path_twice_reuses_the_entry() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();

    let first = state.open_repository(f.path()).unwrap().repo;
    let again = state.open_repository(f.path()).unwrap().repo;

    assert_eq!(first, again);
    assert_eq!(state.overviews().len(), 1);
}

#[test]
fn an_overview_carries_what_the_tree_row_shows() {
    let f = test_fixtures::with_remote().unwrap();
    std::fs::write(f.path().join("file0.txt"), "dirty\n").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let overview = state
        .overviews()
        .into_iter()
        .find(|o| o.repo == repo)
        .unwrap();

    assert_eq!(overview.branch.as_deref(), Some("main"));
    assert_eq!(overview.ahead, 2);
    assert_eq!(overview.behind, 1);
    assert!(overview.dirty);
}

#[test]
fn a_clean_repository_is_not_reported_as_dirty() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(!state.overviews()[0].dirty);
}

#[test]
fn closing_a_repository_removes_it_from_the_list() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let first = state.open_repository(a.path()).unwrap().repo;
    state.open_repository(b.path()).unwrap();

    assert!(state.close_repository(first));

    assert_eq!(state.overviews().len(), 1);
    assert!(state.overviews().iter().all(|o| o.repo != first));
}

#[test]
fn closing_a_repository_leaves_the_files_on_disk() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state.close_repository(repo);

    assert!(f.path().join("file0.txt").exists());
}

#[test]
fn closing_something_that_is_not_open_is_not_an_error() {
    let state = AppState::new();
    assert!(!state.close_repository(app_state::RepoId(9)));
}

#[test]
fn closing_stops_watching_it() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let mut events = state.subscribe();
    state.close_repository(repo);
    while events.try_recv().is_ok() {}

    std::fs::write(f.path().join("file0.txt"), "changed after closing\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));

    assert!(
        events.try_recv().is_err(),
        "a closed repository must not keep an OS watch alive"
    );
}

#[test]
fn overviews_are_sorted_by_name() {
    let names = [
        test_fixtures::linear(1).unwrap(),
        test_fixtures::linear(2).unwrap(),
    ];
    let state = AppState::new();
    for f in &names {
        state.open_repository(f.path()).unwrap();
    }

    let listed: Vec<String> = state.overviews().into_iter().map(|o| o.name).collect();
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
}

#[test]
fn an_overview_of_an_empty_repository_has_no_branch_yet() {
    let f = test_fixtures::empty().unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(state.overviews()[0].branch.is_none());
}
