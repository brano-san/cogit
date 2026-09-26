// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A deleted text file reads as a deletion — every line removed — wherever Diff shows it,
//! never as a size going to nothing (item 29 of 25.09).

use app_state::AppState;
use diff_engine::{DiffOptions, DiffRow, FileDiff};
use git_engine::DiffSpec;

const TEXT: &str = "# Notes\n\nline one\nline two\n";

fn deleted_lines(diff: &FileDiff) -> Vec<String> {
    let FileDiff::Text { hunks, .. } = diff else {
        panic!("expected every line deleted, got {diff:?}");
    };
    hunks
        .iter()
        .flat_map(|hunk| &hunk.rows)
        .map(|row| match row {
            DiffRow::Delete { text, .. } => text.clone(),
            other => panic!("expected only deleted lines, got {other:?}"),
        })
        .collect()
}

#[test]
fn a_deleted_text_file_shows_every_line_removed_on_every_side() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("doc/notes.md", TEXT).unwrap();
    f.write_file("src/lib.rs", TEXT).unwrap();
    f.git(&["add", "--", "."]).unwrap();
    f.commit_staged(1, "add").unwrap();
    f.git(&["rm", "-q", "doc/notes.md"]).unwrap();
    f.commit_staged(2, "remove").unwrap();
    std::fs::remove_file(f.path().join("src/lib.rs")).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let options = DiffOptions::default();
    let head = f.oid("HEAD").unwrap();
    let want: Vec<String> = TEXT.lines().map(str::to_owned).collect();

    let commit = state
        .diff_file(
            repo,
            &DiffSpec::CommitVsParent { oid: head.clone() },
            "doc/notes.md",
            &options,
        )
        .unwrap();
    let worktree = state
        .diff_file(repo, &DiffSpec::WorkTreeVsIndex, "src/lib.rs", &options)
        .unwrap();
    let against_disk = state
        .diff_file(
            repo,
            &DiffSpec::CommitVsWorkTree { oid: head },
            "src/lib.rs",
            &options,
        )
        .unwrap();
    f.git(&["rm", "-q", "--cached", "src/lib.rs"]).unwrap();
    let staged = state
        .diff_file(repo, &DiffSpec::IndexVsHead, "src/lib.rs", &options)
        .unwrap();

    for (name, diff) in [
        ("commit", commit),
        ("working tree", worktree),
        ("commit against the working tree", against_disk),
        ("staged", staged),
    ] {
        assert_eq!(deleted_lines(&diff), want, "{name}");
    }
}
