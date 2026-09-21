// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What reaches the error window and the clipboard after git and its hooks are done
//! colouring, redrawing and printing credentials.

use git_engine::output_text::{collapse_progress, normalise, redact_secrets, strip_ansi, trim};

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

// --- what the renderer is allowed to be handed -------------------------------------

#[test]
fn a_log_shorter_than_the_limit_is_handed_over_byte_for_byte() {
    let log = "warning: LF will be replaced by CRLF\n\n  indented\n";
    assert_eq!(trim(log), log);
}

#[test]
fn a_giant_log_keeps_its_head_and_its_tail() {
    let log: String = (0..30_000).map(|i| format!("line {i}\n")).collect();

    let short = trim(&log);

    assert!(short.starts_with("line 0\nline 1\n"), "{}", &short[..40]);
    assert!(
        short.ends_with("line 29999\n"),
        "{}",
        &short[short.len() - 40..]
    );
}

#[test]
fn the_middle_of_a_giant_log_says_how_much_is_missing() {
    let log: String = (0..30_000).map(|i| format!("line {i}\n")).collect();

    let short = trim(&log);

    assert!(
        short.contains("… 23000 lines omitted, see log …\n"),
        "{short:.400}"
    );
    assert!(!short.contains("line 15000\n"), "the middle should be gone");
}

#[test]
fn the_kept_lines_are_the_ones_promised() {
    let log: String = (0..30_000).map(|i| format!("line {i}\n")).collect();

    let short = trim(&log);

    assert!(short.contains("line 1999\n"));
    assert!(!short.contains("line 2000\n"));
    assert!(!short.contains("line 24999\n"));
    assert!(short.contains("line 25000\n"));
}

#[test]
fn a_panic_at_the_end_of_a_huge_test_log_survives() {
    let mut log: String = (0..40_000).map(|i| format!("test {i} ... ok\n")).collect();
    log.push_str("thread 'main' panicked at crates/a/src/b.rs:12:5:\n");

    let short = trim(&log);

    assert!(short.contains("thread 'main' panicked at crates/a/src/b.rs:12:5:\n"));
}

#[test]
fn a_few_enormous_lines_are_cut_by_bytes_as_well() {
    let log = format!("{}\n{}\n", "x".repeat(2_000_000), "y".repeat(2_000_000));

    let short = trim(&log);

    assert!(short.len() < 2_200_000, "{} bytes", short.len());
    assert!(short.contains("bytes omitted, see log …"), "{:.200}", short);
}

#[test]
fn cutting_by_bytes_never_splits_a_character() {
    // Three bytes per character, so neither cut lands on a boundary by luck. Slicing a
    // `str` off a boundary panics, which is what this test is really watching for.
    let log = "日".repeat(1_400_000);

    let short = trim(&log);

    assert!(short.chars().filter(|c| *c == '日').count() > 600_000);
}
