// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use diff_engine::{DiffOptions, FileDiff};
use git_engine::DiffSpec;

fn head_vs_parent(f: &test_fixtures::Fixture) -> DiffSpec {
    DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    }
}

#[test]
fn diffs_a_file_between_a_commit_and_its_parent() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "content 0\nextra\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.commit_staged(1, "append a line").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "file0.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    match diff {
        FileDiff::Text { hunks, .. } => {
            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].new_lines, 2);
        }
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn an_added_file_shows_every_line_as_new() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "file0.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    match diff {
        FileDiff::Text { hunks, .. } => {
            assert_eq!(hunks[0].old_lines, 0);
            assert_eq!(hunks[0].new_lines, 1);
        }
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn the_language_hint_comes_from_the_path() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("main.rs"), "fn main() {}\n").unwrap();
    f.git(&["add", "--", "main.rs"]).unwrap();
    f.commit_staged(1, "add main.rs").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "main.rs",
            &DiffOptions::default(),
        )
        .unwrap();

    match diff {
        FileDiff::Text { language, .. } => assert_eq!(language.as_deref(), Some("rust")),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn a_crlf_only_change_is_not_shown_as_a_rewrite() {
    let f = test_fixtures::crlf_files().unwrap();
    f.write_file("crlf.txt", "first\nsecond\nthird\n").unwrap();
    f.git(&["add", "--", "crlf.txt"]).unwrap();
    f.commit_staged(1, "convert crlf.txt to LF").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "crlf.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    assert!(
        matches!(diff, FileDiff::EolOnly { .. }),
        "INV-08: expected an EOL-only verdict, got {diff:?}"
    );
}

#[test]
fn a_binary_file_is_summarised_rather_than_rendered() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("blob.bin"), [0_u8, 1, 2, 3]).unwrap();
    f.git(&["add", "--", "blob.bin"]).unwrap();
    f.commit_staged(1, "add a binary file").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "blob.bin",
            &DiffOptions::default(),
        )
        .unwrap();

    assert!(
        matches!(diff, FileDiff::Binary { .. }),
        "expected Binary, got {diff:?}"
    );
}

#[test]
fn an_unknown_repository_is_an_error() {
    let state = AppState::new();
    let spec = DiffSpec::CommitVsParent {
        oid: "HEAD".to_owned(),
    };

    assert!(
        state
            .diff_file(RepoId(7), &spec, "any.txt", &DiffOptions::default())
            .is_err()
    );
}

#[test]
fn a_path_in_neither_side_is_an_error_not_an_empty_diff() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    assert!(
        state
            .diff_file(
                repo,
                &head_vs_parent(&f),
                "never-existed.txt",
                &DiffOptions::default()
            )
            .is_err()
    );
}

#[test]
fn a_hunk_header_names_the_function_it_is_inside() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (0..12).map(|i| format!("    let v{i} = {i};\n")).collect();
    f.write_file("main.rs", &format!("fn outer() {{\n{body}}}\n"))
        .unwrap();
    f.git(&["add", "--", "main.rs"]).unwrap();
    f.commit_staged(1, "add main.rs").unwrap();
    f.write_file(
        "main.rs",
        &format!(
            "fn outer() {{\n{}}}\n",
            body.replace("let v6 = 6;", "let v6 = 66;")
        ),
    )
    .unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let diff = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "main.rs",
            &DiffOptions::default(),
        )
        .unwrap();

    match diff {
        FileDiff::Text { hunks, .. } => {
            assert!(
                hunks[0].header.ends_with("fn outer() {"),
                "{:?}",
                hunks[0].header
            );
        }
        other => panic!("expected a text diff, got {other:?}"),
    }
}
