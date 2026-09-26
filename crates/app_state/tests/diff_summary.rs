// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A file Diff does not show as lines says why, how large each side is and which object
//! each side is, as SmartGit does (R-531).

use app_state::AppState;
use diff_engine::{BinaryCause, BlobSide, DiffOptions, DiffSide, FileDiff};
use git_engine::DiffSpec;

fn rev(f: &test_fixtures::Fixture, spec: &str) -> String {
    f.git(&["rev-parse", spec]).unwrap().trim().to_owned()
}

fn diff(f: &test_fixtures::Fixture, spec: &DiffSpec, path: &str) -> FileDiff {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state
        .diff_file(repo, spec, path, &DiffOptions::default())
        .unwrap()
}

#[test]
fn a_control_character_summarises_the_file_with_both_ids() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(1, "data.txt", "hello world\n").unwrap();
    f.write_file("data.txt", "hello\u{2}world\n").unwrap();
    let hashed = f.git(&["hash-object", "--", "data.txt"]).unwrap();

    let shown = diff(&f, &DiffSpec::WorkTreeVsIndex, "data.txt");

    let FileDiff::Binary { old, new, cause } = shown else {
        panic!("expected a binary summary, got {shown:?}");
    };
    assert_eq!(
        old,
        Some(BlobSide {
            size: 12,
            id: Some(rev(&f, ":data.txt"))
        })
    );
    assert_eq!(
        new,
        Some(BlobSide {
            size: 12,
            id: Some(hashed.trim().to_owned())
        })
    );
    assert_eq!(
        cause,
        BinaryCause::Character {
            code: 2,
            line: 1,
            position: 6,
            side: DiffSide::New
        }
    );
}

#[test]
fn a_deleted_binary_file_has_no_new_side() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.dat binary\n").unwrap();
    f.write_file("table.dat", "one\n").unwrap();
    f.git(&["add", "--", ".gitattributes", "table.dat"])
        .unwrap();
    f.commit_staged(1, "add").unwrap();
    let blob = rev(&f, "HEAD:table.dat");
    f.git(&["rm", "-q", "table.dat"]).unwrap();
    f.commit_staged(2, "remove").unwrap();
    let spec = DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    };

    let shown = diff(&f, &spec, "table.dat");

    assert!(
        matches!(&shown, FileDiff::Binary { old: Some(old), new: None, cause: BinaryCause::Attribute { name } }
            if old.id.as_deref() == Some(blob.as_str()) && name == "binary"),
        "{shown:?}"
    );
}

#[test]
fn a_text_attribute_shows_lines_whatever_the_bytes_hold() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.txt text\n").unwrap();
    f.write_file("data.txt", "one\u{2}\n").unwrap();
    f.git(&["add", "--", ".gitattributes", "data.txt"]).unwrap();
    f.commit_staged(1, "add").unwrap();
    f.write_file("data.txt", "two\u{2}\n").unwrap();

    let shown = diff(&f, &DiffSpec::WorkTreeVsIndex, "data.txt");

    assert!(matches!(shown, FileDiff::Text { .. }), "{shown:?}");
}

#[test]
fn a_file_of_a_million_bytes_is_too_large_and_named() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(1, "big.txt", &"a".repeat(1_000_000)).unwrap();
    let spec = DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    };

    let shown = diff(&f, &spec, "big.txt");

    assert!(
        matches!(&shown, FileDiff::TooLarge { old: None, new: Some(new), limit: 1_000_000 }
            if new.size == 1_000_000 && new.id.as_deref() == Some(rev(&f, "HEAD:big.txt").as_str())),
        "{shown:?}"
    );
}
