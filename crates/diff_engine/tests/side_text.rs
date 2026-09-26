// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The whole of each side travels with a diff the frontend highlights: parsed from the
//! hunks alone, a hunk that starts inside a function reads as broken code, and the tokens
//! of its lines come out cut in the wrong places (R-530).

use diff_engine::{DiffOptions, FileDiff, MAX_HIGHLIGHT_LINES, diff_one};

fn texts(diff: &FileDiff) -> (Option<&str>, Option<&str>) {
    match diff {
        FileDiff::Text {
            old_text, new_text, ..
        } => (old_text.as_deref(), new_text.as_deref()),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn a_highlighted_file_carries_both_sides_whole() {
    let old = "fn a() {\r\n    one();\r\n}\r\n";
    let new = "fn a() {\n    two();\n}\n";

    let diff = diff_one(
        "src/a.rs",
        old.as_bytes(),
        new.as_bytes(),
        &DiffOptions::default(),
    );

    assert_eq!(
        texts(&diff),
        (Some("fn a() {\n    one();\n}\n"), Some(new)),
        "the rows quote the lines with LF endings, and so does the text"
    );
}

#[test]
fn bytes_that_are_not_utf8_read_as_the_rows_read_them() {
    let old = b"fn a() {}\n// caf\xe9\n".to_vec();
    let new = b"fn b() {}\n// caf\xe9\n".to_vec();

    let diff = diff_one("src/a.rs", &old, &new, &DiffOptions::default());

    let FileDiff::Text { hunks, .. } = &diff else {
        panic!("expected a text diff, got {diff:?}");
    };
    let quoted = hunks
        .iter()
        .flat_map(|hunk| &hunk.rows)
        .find_map(|row| match row {
            diff_engine::DiffRow::Context { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    let (_, new_text) = texts(&diff);
    assert_eq!(new_text.unwrap().lines().nth(1), Some(quoted.as_str()));
}

#[test]
fn a_side_past_the_highlight_limit_travels_without_its_text() {
    let long: String = (0..=MAX_HIGHLIGHT_LINES)
        .map(|n| format!("let x{n} = {n};\n"))
        .collect();
    let short = "let x = 1;\n";

    let diff = diff_one(
        "src/a.rs",
        long.as_bytes(),
        short.as_bytes(),
        &DiffOptions::default(),
    );

    assert_eq!(texts(&diff), (None, Some(short)));
}

#[test]
fn a_file_no_parser_highlights_travels_without_its_text() {
    for path in ["notes.md", "LICENSE", "config.yaml"] {
        let diff = diff_one(path, b"one\n", b"two\n", &DiffOptions::default());
        assert_eq!(texts(&diff), (None, None), "{path}");
    }
}
