#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The command a paused rebase runs to say whether the step it just applied is sound.
//! Its verdict is information, never a decision: a failing check must not abort anything
//! the user did not ask to abort (M11 T11.3).

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_passing_command_reports_success_and_its_output() {
    let f = test_fixtures::linear(1).unwrap();
    let run = open(&f).run_check("echo all good").unwrap();

    assert_eq!(run.exit_code, Some(0));
    assert!(run.stdout.contains("all good"), "{run:?}");
}

#[test]
fn a_failing_command_reports_its_code_rather_than_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    // The check failing is a normal outcome; only being unable to run it is an error.
    let run = open(&f).run_check("exit 3").unwrap();
    assert_eq!(run.exit_code, Some(3));
}

#[test]
fn stderr_is_kept_whole() {
    let f = test_fixtures::linear(1).unwrap();
    let run = open(&f).run_check("echo trouble 1>&2").unwrap();
    assert!(run.stderr.contains("trouble"), "{run:?}");
}

#[test]
fn the_command_runs_inside_the_repository() {
    let f = test_fixtures::linear(2).unwrap();
    let run = open(&f).run_check("cat file0.txt").unwrap();
    assert!(run.stdout.contains("content 0"), "{run:?}");
}

#[test]
fn an_empty_command_is_refused_before_anything_is_spawned() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).run_check("   ").is_err());
}

#[test]
fn a_command_that_cannot_start_is_an_error_not_a_verdict() {
    let f = test_fixtures::linear(1).unwrap();
    let run = open(&f).run_check("definitely-not-a-program-xyz").unwrap();
    // The shell itself starts fine and reports the missing program, so this is a verdict.
    assert_ne!(run.exit_code, Some(0));
}

#[test]
fn the_run_is_journalled_so_the_output_panel_can_show_it() {
    let f = test_fixtures::linear(1).unwrap();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = std::sync::Arc::clone(&log);
        std::sync::Arc::new(move |entry: git_engine::GitOutput| {
            log.lock().unwrap().push(entry);
        })
    };

    open(&f)
        .with_journal(sink)
        .run_check("echo checked")
        .unwrap();

    let entries = log.lock().unwrap();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert!(entries[0].command.contains("echo checked"), "{entries:?}");
}

#[test]
fn running_a_check_changes_nothing_in_the_repository() {
    let f = test_fixtures::linear(2).unwrap();
    let before = f.git(&["status", "--porcelain"]).unwrap();
    open(&f).run_check("echo nothing to see").unwrap();
    assert_eq!(f.git(&["status", "--porcelain"]).unwrap(), before);
}
