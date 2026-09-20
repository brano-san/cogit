// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{GitError, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

/// `git -c alias.x='!...' x` runs the alias through a shell, which is the only way to read
/// back the environment the child actually received.
fn echo_env(repo: &RepoHandle, variable: &str) -> String {
    let alias = format!("alias.probe=!printf '%s' \"${variable}\"");
    repo.run_git(&["-c", &alias, "probe"]).unwrap().stdout
}

#[test]
fn runs_in_the_repository_not_in_the_process_directory() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["rev-parse", "--show-toplevel"]).unwrap();

    let reported = std::fs::canonicalize(out.stdout.trim()).unwrap();
    let expected = std::fs::canonicalize(f.path()).unwrap();
    assert_eq!(reported, expected);
}

#[test]
fn captures_stdout_on_success() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["rev-list", "--count", "HEAD"]).unwrap();

    assert_eq!(out.stdout.trim(), "3");
    assert_eq!(out.exit_code, Some(0));
}

#[test]
fn captures_stderr_even_when_the_command_succeeds() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["switch", "-c", "feature"]).unwrap();

    assert!(
        out.stderr.contains("feature"),
        "a successful switch reports on stderr, and that text is what the user needs: {out:?}"
    );
}

#[test]
fn a_failing_command_carries_both_streams_in_full() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let err = repo
        .run_git(&["checkout", "no-such-branch-anywhere"])
        .unwrap_err();

    match err {
        GitError::Command(details) => {
            assert_eq!(details.exit_code, Some(1));
            assert!(details.command.contains("checkout"));
            assert!(
                details.stderr.contains("no-such-branch-anywhere"),
                "INV-05: Git's own words must survive untouched, got {:?}",
                details.stderr
            );
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn the_command_line_is_recorded_for_the_output_panel() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["status", "--porcelain"]).unwrap();

    assert_eq!(out.command, "git status --porcelain");
}

#[test]
fn arguments_are_passed_as_an_array_not_a_command_line() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("two words.txt"), "body\n").unwrap();
    f.git(&["add", "--", "two words.txt"]).unwrap();
    f.commit_staged(1, "add a file whose name has a space")
        .unwrap();
    let repo = open(&f);

    let out = repo
        .run_git(&["log", "--oneline", "--", "two words.txt"])
        .unwrap();

    assert_eq!(out.stdout.lines().count(), 1, "{out:?}");
}

#[test]
fn a_branch_name_that_looks_like_a_flag_is_not_treated_as_one() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let err = repo.run_git(&["rev-parse", "--verify", "--not-a-ref"]);

    assert!(err.is_err(), "a bogus ref must fail, not silently succeed");
}

#[test]
fn terminal_prompting_is_disabled() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert_eq!(echo_env(&repo, "GIT_TERMINAL_PROMPT"), "0");
}

#[test]
fn parsed_output_is_read_in_the_c_locale() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert_eq!(echo_env(&repo, "LC_ALL"), "C");
}

#[test]
fn reads_do_not_take_the_optional_index_lock() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let alias = "alias.probe=!printf '%s' \"$GIT_OPTIONAL_LOCKS\"";
    let out = repo.run_git_reading(&["-c", alias, "probe"]).unwrap();

    assert_eq!(out.stdout, "0");
}

#[test]
fn inherited_git_variables_are_cleared() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
    ] {
        assert_eq!(
            echo_env(&repo, variable),
            "",
            "{variable} must be unset, not overridden (doc/12-risks.md, R-22)"
        );
    }
}

#[test]
fn an_oversized_stream_is_capped_with_a_visible_marker() {
    let f = test_fixtures::linear(1).unwrap();
    let big = "x".repeat(2 * 1024 * 1024);
    std::fs::write(f.path().join("big.txt"), &big).unwrap();
    f.git(&["add", "--", "big.txt"]).unwrap();
    f.commit_staged(1, "add a large file").unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["show", "HEAD:big.txt"]).unwrap();

    assert!(
        out.stdout.contains("truncated by Cogit"),
        "output was not capped"
    );
    assert!(out.stdout.len() < big.len());
}

#[test]
fn the_duration_is_measured() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let out = repo.run_git(&["rev-parse", "HEAD"]).unwrap();

    assert!(out.duration_ms < 60_000);
}

#[test]
fn no_command_can_block_waiting_for_an_editor() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert_eq!(echo_env(&repo, "GIT_EDITOR"), "true");
    assert_eq!(echo_env(&repo, "GIT_SEQUENCE_EDITOR"), "true");
}
