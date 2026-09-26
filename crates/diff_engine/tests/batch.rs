// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A commit's files diff in one parallel pass (doc/08-diff-engine.md section 9).

use diff_engine::{DiffOptions, FileDiff, FileInput, diff_many, diff_one};

fn input(path: &str, old: &str, new: &str) -> FileInput {
    FileInput {
        path: path.to_owned(),
        old: old.as_bytes().to_vec(),
        new: new.as_bytes().to_vec(),
        content: diff_engine::Content::Detect,
    }
}

fn hunks(diff: &FileDiff) -> usize {
    match diff {
        FileDiff::Text { hunks, .. } => hunks.len(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn a_batch_returns_one_diff_per_file_in_input_order() {
    let files = vec![
        input("c.rs", "one\n", "ONE\n"),
        input("a.rs", "two\n", "TWO\n"),
        input("b.rs", "three\n", "THREE\n"),
    ];

    let out = diff_many(files, &DiffOptions::default());

    let paths: Vec<&str> = out.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(
        paths,
        ["c.rs", "a.rs", "b.rs"],
        "the batch must keep the caller's order, not sort it"
    );
}

#[test]
fn a_file_in_a_batch_diffs_exactly_as_it_would_alone() {
    let options = DiffOptions::default();
    let old = "fn main() {\n    one();\n    two();\n}\n";
    let new = "fn main() {\n    one();\n    three();\n}\n";

    let alone = diff_one("src/main.rs", old.as_bytes(), new.as_bytes(), &options);
    let batch = diff_many(vec![input("src/main.rs", old, new)], &options);

    assert_eq!(batch.len(), 1);
    assert_eq!(format!("{:?}", batch[0].diff), format!("{alone:?}"));
}

#[test]
fn the_language_hint_is_filled_in_for_every_file() {
    let out = diff_many(
        vec![
            input("src/main.rs", "a\n", "b\n"),
            input("web/app.py", "a\n", "b\n"),
        ],
        &DiffOptions::default(),
    );

    let languages: Vec<Option<&str>> = out
        .iter()
        .map(|entry| match &entry.diff {
            FileDiff::Text { language, .. } => language.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(languages, [Some("rust"), Some("python")]);
}

#[test]
fn an_unchanged_file_does_not_become_a_text_diff() {
    let out = diff_many(
        vec![input("same.rs", "one\n", "one\n")],
        &DiffOptions::default(),
    );

    assert!(matches!(out[0].diff, FileDiff::Unchanged), "{:?}", out[0]);
}

#[test]
fn an_empty_batch_produces_an_empty_result() {
    let out = diff_many(Vec::new(), &DiffOptions::default());

    assert!(out.is_empty());
}

#[test]
fn a_binary_file_beside_text_files_does_not_derail_the_batch() {
    let files = vec![
        input("a.rs", "one\n", "two\n"),
        FileInput {
            path: "logo.bin".to_owned(),
            old: vec![0, 1, 2, 3],
            new: vec![0, 4, 5, 6],
            content: diff_engine::Content::Detect,
        },
        input("b.rs", "three\n", "four\n"),
    ];

    let out = diff_many(files, &DiffOptions::default());

    assert_eq!(hunks(&out[0].diff), 1);
    assert!(
        matches!(out[1].diff, FileDiff::Binary { .. }),
        "{:?}",
        out[1]
    );
    assert_eq!(hunks(&out[2].diff), 1);
}
