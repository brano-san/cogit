// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Moving a block inside a file and moving it to another file are different facts, and the
//! viewer draws them differently. Cross-file detection needs the whole batch, so it runs
//! after the per-file pass rather than inside it.

use diff_engine::{DiffOptions, DiffRow, FileDiff, FileDiffEntry, FileInput, MoveScope, diff_many};

/// A block long enough to count as a move.
fn block() -> String {
    "fn helper() {\n    one();\n    two();\n    three();\n}\n".to_owned()
}

fn filler(tag: &str) -> String {
    (0..10).map(|i| format!("{tag} line {i}\n")).collect()
}

fn input(path: &str, old: String, new: String) -> FileInput {
    FileInput {
        path: path.to_owned(),
        old: old.into_bytes(),
        new: new.into_bytes(),
    }
}

/// `(path, text, move_id, scope)` for every row that carries a move.
fn moves(entries: &[FileDiffEntry]) -> Vec<(String, String, u32, MoveScope)> {
    let mut out = Vec::new();
    for entry in entries {
        let FileDiff::Text { hunks, .. } = &entry.diff else {
            continue;
        };
        for hunk in hunks {
            for row in &hunk.rows {
                let (text, id, scope) = match row {
                    DiffRow::Delete {
                        text,
                        move_id,
                        move_scope,
                        ..
                    }
                    | DiffRow::Insert {
                        text,
                        move_id,
                        move_scope,
                        ..
                    } => (text, *move_id, *move_scope),
                    _ => continue,
                };
                if let (Some(id), Some(scope)) = (id, scope) {
                    out.push((entry.path.clone(), text.clone(), id, scope));
                }
            }
        }
    }
    out
}

#[test]
fn a_block_moved_inside_one_file_is_scoped_within_file() {
    let rest = filler("a");
    let files = vec![input(
        "a.rs",
        format!("{}{rest}", block()),
        format!("{rest}{}", block()),
    )];

    let out = diff_many(files, &DiffOptions::default());

    let found = moves(&out);
    assert!(!found.is_empty(), "the move should be found at all");
    assert!(
        found
            .iter()
            .all(|(_, _, _, scope)| *scope == MoveScope::WithinFile),
        "{found:#?}"
    );
}

#[test]
fn a_block_moved_between_two_files_is_scoped_across_files() {
    let a = filler("a");
    let b = filler("b");
    let files = vec![
        input("a.rs", format!("{}{a}", block()), a.clone()),
        input("b.rs", b.clone(), format!("{b}{}", block())),
    ];

    let out = diff_many(files, &DiffOptions::default());

    let found = moves(&out);
    assert!(
        !found.is_empty(),
        "a block that left one file for another is a move"
    );
    assert!(
        found
            .iter()
            .all(|(_, _, _, scope)| *scope == MoveScope::AcrossFiles),
        "{found:#?}"
    );
}

#[test]
fn a_cross_file_move_pairs_the_two_files_by_identifier() {
    let a = filler("a");
    let b = filler("b");
    let files = vec![
        input("a.rs", format!("{}{a}", block()), a.clone()),
        input("b.rs", b.clone(), format!("{b}{}", block())),
    ];

    let out = diff_many(files, &DiffOptions::default());

    let found = moves(&out);
    let id = found[0].2;
    let paths: std::collections::BTreeSet<&str> = found
        .iter()
        .filter(|(_, _, found_id, _)| *found_id == id)
        .map(|(path, _, _, _)| path.as_str())
        .collect();
    assert_eq!(
        paths.into_iter().collect::<Vec<_>>(),
        ["a.rs", "b.rs"],
        "one identifier must reach both ends: {found:#?}"
    );
}

#[test]
fn identifiers_are_unique_across_the_batch() {
    let a = filler("a");
    let b = filler("b");
    let files = vec![
        input("a.rs", format!("{}{a}", block()), format!("{a}{}", block())),
        input("b.rs", format!("{}{b}", block()), format!("{b}{}", block())),
    ];

    let out = diff_many(files, &DiffOptions::default());

    let found = moves(&out);
    for (path, _, id, _) in &found {
        let others: Vec<&String> = found
            .iter()
            .filter(|(_, _, other, _)| other == id)
            .map(|(other_path, _, _, _)| other_path)
            .filter(|other_path| *other_path != path)
            .collect();
        assert!(
            others.is_empty(),
            "identifier {id} leaked from {path} into {others:?}: {found:#?}"
        );
    }
}

#[test]
fn a_batch_without_moves_marks_nothing() {
    let files = vec![
        input("a.rs", "one\ntwo\n".to_owned(), "one\nTWO\n".to_owned()),
        input("b.rs", "three\n".to_owned(), "THREE\n".to_owned()),
    ];

    let out = diff_many(files, &DiffOptions::default());

    assert!(moves(&out).is_empty());
}
