// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{DiffOptions, DiffRow, FileDiff, LineEnding, diff_text};

fn hunks(diff: &FileDiff) -> &[diff_engine::Hunk] {
    match diff {
        FileDiff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

fn render(diff: &FileDiff) -> String {
    let mut out = String::new();
    for hunk in hunks(diff) {
        out.push_str(&hunk.header);
        out.push('\n');
        for row in &hunk.rows {
            match row {
                DiffRow::Context { text, .. } => out.push_str(&format!(" {text}\n")),
                DiffRow::Delete { text, .. } => out.push_str(&format!("-{text}\n")),
                DiffRow::Insert { text, .. } => out.push_str(&format!("+{text}\n")),
                DiffRow::Collapsed { count } => out.push_str(&format!("...{count}\n")),
            }
        }
    }
    out
}

#[test]
fn identical_text_produces_no_diff() {
    let diff = diff_text("same\n", "same\n", &DiffOptions::default());
    assert!(matches!(diff, FileDiff::Unchanged));
}

#[test]
fn a_file_that_differs_only_in_line_endings_is_not_a_rewrite() {
    let diff = diff_text("a\nb\nc\n", "a\r\nb\r\nc\r\n", &DiffOptions::default());

    match diff {
        FileDiff::EolOnly { from, to } => {
            assert_eq!(from, LineEnding::Lf);
            assert_eq!(to, LineEnding::Crlf);
        }
        other => panic!("INV-08: expected an EOL-only verdict, got {other:?}"),
    }
}

#[test]
fn line_endings_are_normalized_before_comparing() {
    let diff = diff_text("a\nb\n", "a\r\nCHANGED\r\n", &DiffOptions::default());

    let rendered = render(&diff);
    assert!(rendered.contains("-b"), "{rendered}");
    assert!(rendered.contains("+CHANGED"), "{rendered}");
    assert!(!rendered.contains("-a"), "line a is unchanged: {rendered}");
}

#[test]
fn a_single_edit_becomes_one_hunk_with_context() {
    let old = numbered(20);
    let new = old.replace("line 10\n", "line ten\n");

    let diff = diff_text(&old, &new, &DiffOptions::default());
    let hunks = hunks(&diff);

    assert_eq!(hunks.len(), 1);
    assert_eq!(
        hunks[0].old_start, 7,
        "three lines of context before line 10"
    );
    assert_eq!(hunks[0].old_lines, 7);
    assert_eq!(hunks[0].new_start, 7);
    assert_eq!(hunks[0].new_lines, 7);
}

#[test]
fn the_hunk_header_is_unified_diff_format() {
    let old = numbered(20);
    let new = old.replace("line 10\n", "line ten\n");

    let diff = diff_text(&old, &new, &DiffOptions::default());

    assert_eq!(hunks(&diff)[0].header, "@@ -7,7 +7,7 @@");
}

#[test]
fn distant_edits_stay_in_separate_hunks() {
    let old = numbered(40);
    let new = old
        .replace("line 5\n", "line five\n")
        .replace("line 35\n", "line thirty-five\n");

    let diff = diff_text(&old, &new, &DiffOptions::default());

    assert_eq!(hunks(&diff).len(), 2);
}

#[test]
fn edits_closer_than_twice_the_context_merge_into_one_hunk() {
    let old = numbered(20);
    let new = old
        .replace("line 8\n", "line eight\n")
        .replace("line 11\n", "line eleven\n");

    let diff = diff_text(&old, &new, &DiffOptions::default());

    assert_eq!(
        hunks(&diff).len(),
        1,
        "overlapping context must not produce duplicated lines"
    );
}

#[test]
fn a_pure_insertion_reports_zero_old_lines_at_the_right_place() {
    let diff = diff_text("", "hello\n", &DiffOptions::default());
    let hunks = hunks(&diff);

    assert_eq!(hunks[0].old_start, 0);
    assert_eq!(hunks[0].old_lines, 0);
    assert_eq!(hunks[0].new_start, 1);
    assert_eq!(hunks[0].new_lines, 1);
    assert_eq!(hunks[0].header, "@@ -0,0 +1,1 @@");
}

#[test]
fn a_pure_deletion_reports_zero_new_lines() {
    let diff = diff_text("hello\n", "", &DiffOptions::default());
    let hunks = hunks(&diff);

    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_lines, 0);
    assert_eq!(hunks[0].new_start, 0);
}

#[test]
fn row_numbers_follow_the_side_each_row_belongs_to() {
    let old = "a\nb\nc\n";
    let new = "a\nB\nc\n";

    let diff = diff_text(old, new, &DiffOptions::default());
    let rows = &hunks(&diff)[0].rows;

    let deletes: Vec<u32> = rows
        .iter()
        .filter_map(|r| match r {
            DiffRow::Delete { old, .. } => Some(*old),
            _ => None,
        })
        .collect();
    let inserts: Vec<u32> = rows
        .iter()
        .filter_map(|r| match r {
            DiffRow::Insert { new, .. } => Some(*new),
            _ => None,
        })
        .collect();

    assert_eq!(deletes, [2]);
    assert_eq!(inserts, [2]);
}

#[test]
fn context_is_configurable() {
    let old = numbered(20);
    let new = old.replace("line 10\n", "line ten\n");
    let options = DiffOptions {
        context_lines: 1,
        ..DiffOptions::default()
    };

    let diff = diff_text(&old, &new, &options);

    assert_eq!(hunks(&diff)[0].old_start, 9);
    assert_eq!(hunks(&diff)[0].old_lines, 3);
}

#[test]
fn a_file_without_a_trailing_newline_keeps_its_last_line() {
    let diff = diff_text("a\nb", "a\nB", &DiffOptions::default());
    let rendered = render(&diff);

    assert!(rendered.contains("-b"), "{rendered}");
    assert!(rendered.contains("+B"), "{rendered}");
}

#[test]
fn a_binary_file_is_never_rendered_as_text() {
    let diff = diff_engine::diff_bytes(
        b"\x00\x01binary",
        b"\x00\x02binary",
        &DiffOptions::default(),
    );

    assert!(
        matches!(diff, FileDiff::Binary { .. }),
        "expected Binary, got {diff:?}"
    );
}

#[test]
fn invalid_utf8_is_decoded_lossily_rather_than_refused() {
    let diff = diff_engine::diff_bytes(b"caf\xe9\n", b"cafe\n", &DiffOptions::default());

    match diff {
        FileDiff::Text { lossy_encoding, .. } => assert!(lossy_encoding),
        other => panic!("expected a lossy text diff, got {other:?}"),
    }
}

#[test]
fn an_oversized_file_is_refused_before_it_is_read_as_text() {
    let big = vec![b'a'; 2 * 1024 * 1024];
    let diff = diff_engine::diff_bytes(&big, &big[..big.len() - 1], &DiffOptions::default());

    assert!(
        matches!(diff, FileDiff::TooLarge { .. }),
        "expected TooLarge, got {diff:?}"
    );
}

#[test]
fn snapshot_a_multi_hunk_diff() {
    let old = numbered(40);
    let new = old
        .replace("line 3\n", "line three\n")
        .replace("line 20\n", "line twenty\nline twenty and a half\n")
        .replace("line 38\n", "");

    let diff = diff_text(&old, &new, &DiffOptions::default());
    insta::assert_snapshot!(render(&diff));
}

#[test]
fn a_file_with_mixed_line_endings_still_diffs_line_by_line() {
    let old = "a\nb\r\nc\n";
    let new = "a\nCHANGED\r\nc\n";

    let diff = diff_text(old, new, &DiffOptions::default());
    let rendered = render(&diff);

    assert!(rendered.contains("-b"), "{rendered}");
    assert!(rendered.contains("+CHANGED"), "{rendered}");
    assert!(!rendered.contains("-a"), "{rendered}");
    assert!(!rendered.contains("-c"), "{rendered}");
}

#[test]
fn mixed_line_endings_are_reported_as_mixed() {
    let diff = diff_text("a\nb\r\n", "a\nB\r\n", &DiffOptions::default());

    match diff {
        FileDiff::Text { eol, .. } => assert_eq!(eol.old, LineEnding::Mixed),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn one_enormous_line_does_not_hang_the_engine() {
    let old = format!("{}\n", "x".repeat(900_000));
    let new = format!("{}y\n", "x".repeat(900_000));

    let started = std::time::Instant::now();
    let diff = diff_text(&old, &new, &DiffOptions::default());
    let elapsed = started.elapsed();

    assert_eq!(hunks(&diff).len(), 1);
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "minified JS must not stall the engine, took {elapsed:?}"
    );
}

#[test]
fn ignoring_trailing_whitespace_hides_a_whitespace_only_edit() {
    let options = DiffOptions {
        ignore_whitespace: diff_engine::Whitespace::Trailing,
        ..DiffOptions::default()
    };

    let diff = diff_text("a\nb\n", "a\nb   \n", &options);

    assert!(
        matches!(diff, FileDiff::WhitespaceOnly),
        "expected WhitespaceOnly, got {diff:?}"
    );
}

#[test]
fn ignoring_all_whitespace_survives_reindentation() {
    let options = DiffOptions {
        ignore_whitespace: diff_engine::Whitespace::All,
        ..DiffOptions::default()
    };

    let diff = diff_text("if (x) {\n", "    if  (x)  {\n", &options);

    assert!(
        matches!(diff, FileDiff::WhitespaceOnly),
        "expected WhitespaceOnly, got {diff:?}"
    );
}

#[test]
fn a_whitespace_only_change_is_told_apart_from_no_change_at_all() {
    let options = DiffOptions {
        ignore_whitespace: diff_engine::Whitespace::All,
        ..DiffOptions::default()
    };

    let same = diff_text("a\n", "a\n", &options);
    let reindented = diff_text("if (x) {\n", "    if (x) {\n", &options);

    assert!(matches!(same, FileDiff::Unchanged), "got {same:?}");
    assert!(
        matches!(reindented, FileDiff::WhitespaceOnly),
        "the user must know the diff is being filtered, got {reindented:?}"
    );
}

#[test]
fn ignoring_whitespace_still_shows_a_real_change() {
    let options = DiffOptions {
        ignore_whitespace: diff_engine::Whitespace::All,
        ..DiffOptions::default()
    };

    let diff = diff_text("if (x) {\n", "    if (y) {\n", &options);

    assert!(matches!(diff, FileDiff::Text { .. }), "got {diff:?}");
}

#[test]
fn without_the_option_a_reindent_is_an_ordinary_change() {
    let diff = diff_text("if (x) {\n", "    if (x) {\n", &DiffOptions::default());

    assert!(matches!(diff, FileDiff::Text { .. }), "got {diff:?}");
}
