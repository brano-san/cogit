// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! How a finished git command is classified and summed up for the error window.

use git_engine::outcome::{Severity, operation_label, severity_of, summarise};

#[test]
fn a_non_zero_exit_is_a_failure() {
    assert_eq!(severity_of(Some(1), ""), Severity::Failure);
    assert_eq!(severity_of(Some(128), "anything"), Severity::Failure);
}

#[test]
fn a_command_that_did_not_start_is_a_failure() {
    assert_eq!(severity_of(None, ""), Severity::Failure);
}

#[test]
fn a_warning_on_a_successful_command_is_a_warning() {
    assert_eq!(
        severity_of(Some(0), "warning: LF will be replaced by CRLF in a.txt"),
        Severity::Warning,
    );
    assert_eq!(
        severity_of(Some(0), "hint: use --force to override"),
        Severity::Warning
    );
}

#[test]
fn progress_on_a_successful_command_is_not_a_warning() {
    assert_eq!(
        severity_of(Some(0), "Counting objects: 100%\nDone.\n"),
        Severity::Success
    );
}

#[test]
fn a_silent_success_is_a_success() {
    assert_eq!(severity_of(Some(0), ""), Severity::Success);
}

#[test]
fn the_summary_prefers_the_last_line_that_says_what_went_wrong() {
    let stderr = "remote: Resolving deltas: 100%\nerror: failed to push some refs\nfatal: the remote end hung up\n";
    assert_eq!(summarise(stderr, ""), "fatal: the remote end hung up");
}

#[test]
fn the_summary_recognises_rejected_and_failed_to() {
    assert_eq!(
        summarise("! [rejected] main -> main (fetch first)\n", ""),
        "! [rejected] main -> main (fetch first)"
    );
    assert_eq!(
        summarise("failed to run pre-push hook\n", ""),
        "failed to run pre-push hook"
    );
}

#[test]
fn without_a_marker_the_summary_is_the_last_thing_said() {
    assert_eq!(summarise("first\nsecond\n\n", ""), "second");
}

#[test]
fn the_summary_falls_back_to_stdout_when_stderr_is_silent() {
    assert_eq!(
        summarise("", "Everything up-to-date\n"),
        "Everything up-to-date"
    );
}

#[test]
fn the_summary_is_empty_when_nothing_was_said() {
    assert_eq!(summarise("", ""), "");
}

#[test]
fn a_long_line_is_cut_rather_than_wrapped_into_the_title() {
    let long = format!("error: {}", "x".repeat(400));
    let summary = summarise(&long, "");
    assert!(
        summary.chars().count() <= 160,
        "{}",
        summary.chars().count()
    );
    assert!(summary.ends_with('…'), "{summary}");
}

#[test]
fn the_summary_never_runs_past_two_lines() {
    let stderr = "error: one\nerror: two\nerror: three\n";
    let summary = summarise(stderr, "");
    assert!(summary.lines().count() <= 2, "{summary}");
    assert_eq!(summary, "error: three");
}

#[test]
fn the_label_names_the_command_that_actually_ran() {
    assert_eq!(operation_label("git push origin main"), "Push");
    assert_eq!(operation_label("git fetch --all"), "Fetch All");
    assert_eq!(operation_label("git pull --rebase"), "Pull (rebase)");
    assert_eq!(operation_label("git branch -D feature/x"), "Delete Branch");
    assert_eq!(operation_label("git commit -m wip"), "Commit");
}

#[test]
fn a_command_with_leading_config_flags_is_still_recognised() {
    assert_eq!(
        operation_label("git -c core.hooksPath=.githooks push origin main"),
        "Push"
    );
}

#[test]
fn an_unknown_subcommand_is_named_after_itself_rather_than_guessed() {
    assert_eq!(operation_label("git bisect start"), "Bisect");
    assert_eq!(operation_label("git cherry-pick abc"), "Cherry-pick");
}

#[test]
fn something_that_is_not_a_git_command_still_gets_a_name() {
    assert_eq!(operation_label(""), "Git");
    assert_eq!(operation_label("git"), "Git");
}

// Commit What You See runs git on a scratch index, and the journal line says so.
#[test]
fn an_environment_before_git_is_not_taken_for_the_subcommand() {
    assert_eq!(
        operation_label("GIT_INDEX_FILE='D:/my git repo/.git/x' git -c a=b commit -m wip"),
        "Commit"
    );
}
