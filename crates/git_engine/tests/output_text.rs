// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What reaches the error window and the clipboard after git and its hooks are done
//! colouring, redrawing and printing credentials.

use git_engine::output_text::{collapse_progress, normalise, redact_secrets, strip_ansi};

#[test]
fn colour_codes_are_stripped() {
    let coloured = "\u{1b}[31merror:\u{1b}[0m nothing to commit";
    assert_eq!(strip_ansi(coloured), "error: nothing to commit");
}

#[test]
fn a_cursor_move_is_stripped_too() {
    assert_eq!(strip_ansi("a\u{1b}[2Kb"), "ab");
}

#[test]
fn plain_text_survives_untouched() {
    let text = "warning: LF will be replaced by CRLF in src/main.rs\n";
    assert_eq!(strip_ansi(text), text);
}

#[test]
fn a_progress_line_keeps_only_its_last_state() {
    let noisy = "Counting objects:  3%\rCounting objects: 47%\rCounting objects: 100%\n";
    assert_eq!(collapse_progress(noisy), "Counting objects: 100%\n");
}

#[test]
fn progress_on_several_lines_collapses_each_separately() {
    let noisy = "Counting:  1%\rCounting: 100%\nWriting:  5%\rWriting: 100%\n";
    assert_eq!(collapse_progress(noisy), "Counting: 100%\nWriting: 100%\n");
}

#[test]
fn ordinary_newlines_and_indentation_are_untouched() {
    let log = "running 2 tests\n    test a ... ok\n\n    test b ... FAILED\n";
    assert_eq!(collapse_progress(log), log);
}

#[test]
fn a_lone_carriage_return_at_the_end_does_not_eat_the_line() {
    assert_eq!(collapse_progress("done\r"), "done");
}

#[test]
fn a_password_in_a_remote_url_is_hidden() {
    let text = "fatal: could not read from https://brano:ghp_secret123@github.com/x/y.git\n";
    let clean = redact_secrets(text);
    assert!(!clean.contains("ghp_secret123"), "{clean}");
    assert!(
        clean.contains("https://brano:***@github.com/x/y.git"),
        "{clean}"
    );
}

#[test]
fn the_user_and_the_host_survive_so_the_message_still_means_something() {
    let clean = redact_secrets("remote: https://bob:hunter2@example.com/repo");
    assert!(clean.contains("bob"), "{clean}");
    assert!(clean.contains("example.com"), "{clean}");
    assert!(!clean.contains("hunter2"), "{clean}");
}

#[test]
fn an_authorization_header_is_hidden() {
    let clean = redact_secrets("Authorization: Bearer ghp_abcdef123456\n");
    assert!(!clean.contains("ghp_abcdef123456"), "{clean}");
    assert!(clean.contains("Authorization:"), "{clean}");
}

#[test]
fn an_environment_variable_that_names_a_secret_is_hidden() {
    for line in [
        "GITHUB_TOKEN=ghp_xyz",
        "MY_PASSWORD=hunter2",
        "some_secret=abc123",
    ] {
        let clean = redact_secrets(line);
        assert!(!clean.contains("ghp_xyz"), "{clean}");
        assert!(!clean.contains("hunter2"), "{clean}");
        assert!(!clean.contains("abc123"), "{clean}");
    }
}

#[test]
fn a_url_without_credentials_is_left_alone() {
    let text = "remote: https://github.com/x/y.git\n";
    assert_eq!(redact_secrets(text), text);
}

#[test]
fn an_ordinary_assignment_is_left_alone() {
    let text = "CARGO_TARGET_DIR=target-probe\n";
    assert_eq!(redact_secrets(text), text);
}

#[test]
fn normalise_does_all_three_and_keeps_the_shape_of_a_test_log() {
    let raw = "\u{1b}[32mCompiling\u{1b}[0m cogit\rCompiling cogit v0.1.0\n\
               \n    thread 'a' panicked at src/lib.rs:12:5:\n      assertion failed\n";

    let clean = normalise(raw);

    assert!(!clean.contains('\u{1b}'), "{clean}");
    assert!(clean.contains("Compiling cogit v0.1.0\n"), "{clean}");
    assert!(
        clean.contains("\n\n    thread 'a' panicked at src/lib.rs:12:5:\n"),
        "{clean}"
    );
    assert!(clean.contains("      assertion failed\n"), "{clean}");
}

#[test]
fn normalising_empty_output_is_empty() {
    assert_eq!(normalise(""), "");
}
