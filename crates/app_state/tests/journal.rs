// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, JOURNAL_CAPACITY};

#[test]
fn every_git_command_leaves_an_entry() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();

    state.stage_paths(repo, &["fresh.txt".to_owned()]).unwrap();

    let log = state.command_log();
    assert!(
        log.iter().any(|entry| entry.command.contains("add")),
        "got {:?}",
        log.iter().map(|e| &e.command).collect::<Vec<_>>()
    );
}

#[test]
fn a_failed_command_is_recorded_with_its_exit_code() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let _ = state.stage_paths(repo, &["never-existed.txt".to_owned()]);

    let entry = state
        .command_log()
        .into_iter()
        .find(|e| e.command.contains("never-existed"))
        .expect("a failure must still be logged");
    assert_ne!(entry.exit_code, Some(0));
    assert!(!entry.stderr.is_empty());
}

#[test]
fn a_successful_command_keeps_its_stderr() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state.create_branch(repo, "topic", None, true).unwrap();

    let entry = state
        .command_log()
        .into_iter()
        .find(|e| e.command.contains("switch"))
        .unwrap();
    assert_eq!(entry.exit_code, Some(0));
    assert!(
        !entry.stderr.is_empty(),
        "INV-05: a successful switch still says something worth seeing"
    );
}

#[test]
fn the_journal_is_newest_first() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state.create_branch(repo, "one", None, false).unwrap();
    state.create_branch(repo, "two", None, false).unwrap();

    let log = state.command_log();
    let first = log.first().unwrap();
    assert!(first.command.contains("two"), "got {}", first.command);
}

#[test]
fn the_journal_does_not_grow_without_bound() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    for i in 0..(JOURNAL_CAPACITY + 20) {
        state
            .create_branch(repo, &format!("branch-{i}"), None, false)
            .unwrap();
    }

    assert_eq!(state.command_log().len(), JOURNAL_CAPACITY);
}

#[test]
fn a_warning_is_a_zero_exit_code_with_something_on_stderr() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state.create_branch(repo, "topic", None, true).unwrap();

    assert!(state.command_log().iter().any(app_state::is_warning));
}

#[test]
fn clearing_empties_the_journal() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.create_branch(repo, "topic", None, false).unwrap();

    state.clear_command_log();

    assert!(state.command_log().is_empty());
}
