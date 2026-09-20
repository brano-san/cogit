// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{DiffOptions, DiffRow, FileDiff, MIN_MOVED_LINES, detect_moves, diff_text};

fn rows(old: &str, new: &str) -> Vec<DiffRow> {
    let mut diff = diff_text(old, new, &DiffOptions::default());
    detect_moves(&mut diff);
    match diff {
        FileDiff::Text { hunks, .. } => hunks.into_iter().flat_map(|h| h.rows).collect(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn moved(rows: &[DiffRow]) -> usize {
    rows.iter()
        .filter(|row| match row {
            DiffRow::Delete { moved, .. } | DiffRow::Insert { moved, .. } => *moved,
            _ => false,
        })
        .count()
}

/// A block long enough to count, moved from the top of the file to the bottom.
fn moved_block() -> (String, String) {
    let block = "fn helper() {\n    one();\n    two();\n    three();\n}\n";
    let rest: String = (0..10).map(|i| format!("line {i}\n")).collect();
    (format!("{block}{rest}"), format!("{rest}{block}"))
}

#[test]
fn a_moved_block_is_marked_on_both_sides() {
    let (old, new) = moved_block();

    let rows = rows(&old, &new);

    assert!(moved(&rows) >= MIN_MOVED_LINES * 2, "{rows:#?}");
}

#[test]
fn an_ordinary_edit_is_not_called_a_move() {
    let rows = rows("one\ntwo\nthree\n", "one\nTWO\nthree\n");

    assert_eq!(moved(&rows), 0);
}

#[test]
fn a_block_shorter_than_the_threshold_is_not_a_move() {
    let old = "a\nb\nkeep0\nkeep1\nkeep2\nkeep3\n";
    let new = "keep0\nkeep1\nkeep2\nkeep3\na\nb\n";

    assert_eq!(
        moved(&rows(old, new)),
        0,
        "two lines is below the threshold"
    );
}

#[test]
fn indentation_alone_does_not_hide_a_move() {
    let block = "fn helper() {\n    one();\n    two();\n    three();\n}\n";
    let indented: String = block.lines().map(|l| format!("    {l}\n")).collect();
    let rest: String = (0..10).map(|i| format!("line {i}\n")).collect();

    let rows = rows(&format!("{block}{rest}"), &format!("{rest}{indented}"));

    assert!(
        moved(&rows) > 0,
        "Git ignores indentation here too: {rows:#?}"
    );
}

#[test]
fn a_deletion_with_no_matching_addition_stays_a_deletion() {
    let old: String = (0..10).map(|i| format!("line {i}\n")).collect();
    let new: String = (5..10).map(|i| format!("line {i}\n")).collect();

    assert_eq!(moved(&rows(&old, &new)), 0);
}

#[test]
fn detection_leaves_a_diff_that_is_not_text_alone() {
    let mut diff = FileDiff::Unchanged;
    detect_moves(&mut diff);
    assert!(matches!(diff, FileDiff::Unchanged));
}

#[test]
fn the_text_of_a_moved_line_is_untouched() {
    let (old, new) = moved_block();

    let rows = rows(&old, &new);
    let first = rows
        .iter()
        .find(|row| matches!(row, DiffRow::Delete { moved: true, .. }))
        .unwrap();

    match first {
        DiffRow::Delete { text, .. } => assert!(!text.is_empty()),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn two_separate_moves_are_both_found() {
    let first = "fn a() {\n    one();\n    two();\n}\n";
    let second = "fn b() {\n    three();\n    four();\n}\n";
    let rest: String = (0..10).map(|i| format!("line {i}\n")).collect();

    let rows = rows(
        &format!("{first}{second}{rest}"),
        &format!("{rest}{second}{first}"),
    );

    assert!(moved(&rows) >= 8, "{rows:#?}");
}

/// `(text, move_id)` for every row that could carry a pairing.
fn pairings(rows: &[DiffRow]) -> Vec<(String, Option<u32>)> {
    rows.iter()
        .filter_map(|row| match row {
            DiffRow::Delete { text, move_id, .. } | DiffRow::Insert { text, move_id, .. } => {
                Some((text.clone(), *move_id))
            }
            _ => None,
        })
        .collect()
}

fn ids(rows: &[DiffRow]) -> Vec<u32> {
    let mut seen: Vec<u32> = pairings(rows)
        .into_iter()
        .filter_map(|(_, id)| id)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn both_ends_of_a_move_share_one_identifier() {
    let (old, new) = moved_block();

    let rows = rows(&old, &new);

    assert_eq!(ids(&rows).len(), 1, "one move means one identifier");
    let id = ids(&rows)[0];
    let deleted: Vec<String> = rows
        .iter()
        .filter_map(|row| match row {
            DiffRow::Delete { text, move_id, .. } if *move_id == Some(id) => Some(text.clone()),
            _ => None,
        })
        .collect();
    let inserted: Vec<String> = rows
        .iter()
        .filter_map(|row| match row {
            DiffRow::Insert { text, move_id, .. } if *move_id == Some(id) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(deleted, inserted, "both ends carry the same lines");
}

#[test]
fn two_separate_moves_get_two_identifiers() {
    let first = "fn a() {\n    one();\n    two();\n}\n";
    let second = "fn b() {\n    three();\n    four();\n}\n";
    let rest: String = (0..10).map(|i| format!("line {i}\n")).collect();

    let rows = rows(
        &format!("{first}{second}{rest}"),
        &format!("{rest}{second}{first}"),
    );

    assert_eq!(ids(&rows).len(), 2, "{rows:#?}");
}

#[test]
fn an_ordinary_edit_has_no_move_identifier() {
    let rows = rows("one\ntwo\nthree\n", "one\nTWO\nthree\n");

    assert!(
        pairings(&rows).iter().all(|(_, id)| id.is_none()),
        "{rows:#?}"
    );
}

#[test]
fn every_row_marked_moved_carries_an_identifier() {
    let (old, new) = moved_block();

    let rows = rows(&old, &new);

    for row in &rows {
        let (flagged, id) = match row {
            DiffRow::Delete { moved, move_id, .. } | DiffRow::Insert { moved, move_id, .. } => {
                (*moved, *move_id)
            }
            _ => continue,
        };
        assert_eq!(flagged, id.is_some(), "{row:?}");
    }
}
