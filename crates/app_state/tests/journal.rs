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
    git_engine::GitOutput::record(
        std::path::Path::new("."),
        command.to_owned(),
        Some(0),
        "",
        "",
        0,
    )
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

#[test]
fn one_entry_can_be_fetched_by_its_number() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.create_branch(repo, "topic", None, false).unwrap();

    let wanted = state.command_log().first().unwrap().id;

    assert_eq!(state.command_outcome(wanted).unwrap().id, wanted);
}

fn ran(
    command: &str,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    ms: u32,
) -> git_engine::GitOutput {
    git_engine::GitOutput::record(
        std::path::Path::new("."),
        command.to_owned(),
        exit_code,
        stdout,
        stderr,
        ms,
    )
}

fn journal_of(
    entries: Vec<git_engine::GitOutput>,
) -> std::collections::VecDeque<git_engine::GitOutput> {
    let mut log = std::collections::VecDeque::new();
    for entry in entries {
        app_state::record(&mut log, 100, entry);
    }
    log
}

#[test]
fn the_rows_are_newest_first_and_flag_warnings_like_the_full_entries() {
    let log = journal_of(vec![
        ran(
            "git switch topic",
            Some(0),
            "",
            "Switched to branch 'topic'",
            1,
        ),
        ran("git status", Some(0), "On branch master", "", 1),
        ran(
            "git add gone.txt",
            Some(1),
            "",
            "fatal: pathspec did not match",
            1,
        ),
    ]);

    let rows = app_state::command_rows(&log);

    let ids: Vec<u32> = rows.iter().map(|row| row.id).collect();
    let newest_first: Vec<u32> = log.iter().rev().map(|entry| entry.id).collect();
    assert_eq!(ids, newest_first);
    let warned: Vec<bool> = rows.iter().map(|row| row.warned).collect();
    assert_eq!(warned, [false, false, true]);
    let expected: Vec<bool> = log.iter().rev().map(app_state::is_warning).collect();
    assert_eq!(warned, expected);
}

/// What the panel's Copy log put on the clipboard when it built the text itself.
#[test]
fn the_log_text_reads_as_the_terminal_would_have_shown_it() {
    let log = journal_of(vec![
        ran("git status", Some(0), "On branch master", "", 5),
        ran(
            "git switch topic",
            Some(0),
            "",
            "Switched to branch 'topic'",
            7,
        ),
        ran("git push", None, "  \n", "fatal: no remote", 9),
    ]);

    assert_eq!(
        app_state::command_log_text(&log, false),
        "$ git push\nexit ? in 9 ms\nfatal: no remote\n\n\
         $ git switch topic\nexit 0 in 7 ms\nSwitched to branch 'topic'\n\n\
         $ git status\nexit 0 in 5 ms\nOn branch master"
    );
    assert_eq!(
        app_state::command_log_text(&log, true),
        "$ git push\nexit ? in 9 ms\nfatal: no remote\n\n\
         $ git switch topic\nexit 0 in 7 ms\nSwitched to branch 'topic'"
    );
}

/// The window is opened from a notice that outlives the entry it points at; asking for
/// one that has already rotated out of the ring must read as "gone", not as a failure.
#[test]
fn asking_for_an_entry_that_has_rotated_out_is_not_an_error() {
    let state = AppState::new();
    assert!(state.command_outcome(999_999).is_none());
}
