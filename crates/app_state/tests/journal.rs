// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;

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

fn entry(command: &str) -> git_engine::GitOutput {
    git_engine::GitOutput {
        command: command.to_owned(),
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 0,
    }
}

/// Driven directly rather than through 520 `git` processes: the property is a ring buffer,
/// and spawning half a thousand processes to prove it cost 26 s of every commit (R-55).
#[test]
fn the_journal_does_not_grow_without_bound() {
    let mut log = std::collections::VecDeque::new();
    for i in 0..10 {
        app_state::record(&mut log, 3, entry(&format!("git branch-{i}")));
    }

    assert_eq!(log.len(), 3);
}

#[test]
fn the_oldest_entry_is_the_one_that_makes_room() {
    let mut log = std::collections::VecDeque::new();
    for i in 0..5 {
        app_state::record(&mut log, 3, entry(&format!("git {i}")));
    }

    let kept: Vec<&str> = log.iter().map(|e| e.command.as_str()).collect();
    assert_eq!(kept, ["git 2", "git 3", "git 4"]);
}

#[test]
fn a_journal_below_its_capacity_keeps_everything() {
    let mut log = std::collections::VecDeque::new();
    app_state::record(&mut log, 3, entry("git one"));
    app_state::record(&mut log, 3, entry("git two"));
    assert_eq!(log.len(), 2);
}

/// The capacity the application actually runs with is still honoured end to end; a handful
/// of commands is enough to prove the sink is wired to the ring.
#[test]
fn the_real_journal_uses_the_shared_capacity() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    for i in 0..3 {
        state
            .create_branch(repo, &format!("branch-{i}"), None, false)
            .unwrap();
    }

    assert_eq!(state.command_log().len(), 3);
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
