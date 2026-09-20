// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `\ No newline at end of file`: the fact a unified diff prints under the last line of
//! whichever side lacks it. `build_patch` already emits the marker; these tests are about
//! the viewer, which had no way to know.

use diff_engine::{DiffOptions, DiffRow, FileDiff, diff_text};

fn hunks(diff: &FileDiff) -> Vec<diff_engine::Hunk> {
    match diff {
        FileDiff::Text { hunks, .. } => hunks.clone(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn rows(diff: &FileDiff) -> Vec<DiffRow> {
    hunks(diff).into_iter().flat_map(|hunk| hunk.rows).collect()
}

/// `(text, no_newline)` for every row that carries the flag.
fn flags(diff: &FileDiff) -> Vec<(String, bool)> {
    rows(diff)
        .into_iter()
        .filter_map(|row| match row {
            DiffRow::Delete {
                text, no_newline, ..
            }
            | DiffRow::Insert {
                text, no_newline, ..
            } => Some((text, no_newline)),
            _ => None,
        })
        .collect()
}

#[test]
fn adding_a_final_newline_is_a_change_rather_than_nothing() {
    let diff = diff_text("alpha\nbeta", "alpha\nbeta\n", &DiffOptions::default());

    assert!(
        !matches!(diff, FileDiff::Unchanged),
        "a file that gained its final newline did change: {diff:?}"
    );
}

#[test]
fn removing_a_final_newline_is_a_change_rather_than_nothing() {
    let diff = diff_text("alpha\nbeta\n", "alpha\nbeta", &DiffOptions::default());

    assert!(
        !matches!(diff, FileDiff::Unchanged),
        "a file that lost its final newline did change: {diff:?}"
    );
}

#[test]
fn the_side_that_lacks_the_newline_is_the_one_flagged() {
    let diff = diff_text("alpha\nbeta", "alpha\ngamma\n", &DiffOptions::default());

    assert_eq!(
        flags(&diff),
        vec![("beta".to_owned(), true), ("gamma".to_owned(), false)]
    );
}

#[test]
fn both_sides_can_lack_the_final_newline() {
    let diff = diff_text("alpha\nbeta", "alpha\ngamma", &DiffOptions::default());

    assert_eq!(
        flags(&diff),
        vec![("beta".to_owned(), true), ("gamma".to_owned(), true)]
    );
}

#[test]
fn a_file_ending_in_a_newline_flags_nothing() {
    let diff = diff_text("alpha\nbeta\n", "alpha\ngamma\n", &DiffOptions::default());

    assert!(
        flags(&diff).iter().all(|(_, flagged)| !flagged),
        "{:?}",
        flags(&diff)
    );
}

#[test]
fn only_the_last_line_carries_the_marker() {
    let old = "one\ntwo\nthree";
    let new = "ONE\nTWO\nTHREE";

    let diff = diff_text(old, new, &DiffOptions::default());

    let flagged: Vec<String> = flags(&diff)
        .into_iter()
        .filter(|(_, marked)| *marked)
        .map(|(text, _)| text)
        .collect();
    assert_eq!(flagged, ["three", "THREE"]);
}

#[test]
fn a_change_far_from_the_end_flags_nothing_even_without_a_final_newline() {
    let old = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight";
    let new = "one\nTWO\nthree\nfour\nfive\nsix\nseven\neight";

    let diff = diff_text(old, new, &DiffOptions::default());

    assert!(
        flags(&diff).iter().all(|(_, flagged)| !flagged),
        "the last line is untouched, so no row should carry the marker: {:?}",
        flags(&diff)
    );
}
