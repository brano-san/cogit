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

#[test]
fn move_detection_can_be_turned_off() {
    let block = "alpha\nbeta\ngamma\n";
    let body = "one\ntwo\nthree\nfour\nfive\n";
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("moved.txt"), format!("{block}{body}")).unwrap();
    f.git(&["add", "--", "moved.txt"]).unwrap();
    f.commit_staged(1, "add a block").unwrap();
    std::fs::write(f.path().join("moved.txt"), format!("{body}{block}")).unwrap();
    f.git(&["add", "--", "moved.txt"]).unwrap();
    f.commit_staged(2, "move the block").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let options = DiffOptions {
        detect_moves: false,
        ..DiffOptions::default()
    };

    let diff = state
        .diff_file(repo, &head_vs_parent(&f), "moved.txt", &options)
        .unwrap();

    match diff {
        FileDiff::Text { hunks, .. } => assert!(
            hunks
                .iter()
                .flat_map(|hunk| &hunk.rows)
                .all(|row| !matches!(
                    row,
                    diff_engine::DiffRow::Delete { moved: true, .. }
                        | diff_engine::DiffRow::Insert { moved: true, .. }
                )),
            "{hunks:?}"
        ),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn move_detection_is_on_by_default() {
    let block = "alpha\nbeta\ngamma\n";
    let body = "one\ntwo\nthree\nfour\nfive\n";
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("moved.txt"), format!("{block}{body}")).unwrap();
    f.git(&["add", "--", "moved.txt"]).unwrap();
    f.commit_staged(1, "add a block").unwrap();
    std::fs::write(f.path().join("moved.txt"), format!("{body}{block}")).unwrap();
    f.git(&["add", "--", "moved.txt"]).unwrap();
    f.commit_staged(2, "move the block").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &head_vs_parent(&f),
            "moved.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    match diff {
        FileDiff::Text { hunks, .. } => assert!(
            hunks.iter().flat_map(|hunk| &hunk.rows).any(|row| matches!(
                row,
                diff_engine::DiffRow::Delete { moved: true, .. }
                    | diff_engine::DiffRow::Insert { moved: true, .. }
            )),
            "{hunks:?}"
        ),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

/// A submodule that has not been checked out is a normal state of a repository, not a
/// broken one. SmartGit shows "Submodule does not exist!" on both sides; Cogit used to
/// answer `Invalid repository state` and show nothing at all.
#[test]
fn a_submodule_that_is_not_checked_out_is_described_rather_than_refused() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let head = f.oid("HEAD").unwrap();

    let shown = state
        .diff_file(
            repo,
            &git_engine::DiffSpec::CommitVsParent { oid: head },
            "vendor/lib",
            &diff_engine::DiffOptions::default(),
        )
        .expect("a missing submodule must not be an error");

    match shown {
        diff_engine::FileDiff::Submodule { recorded, .. } => {
            assert!(!recorded.is_empty(), "the recorded pointer is the diff");
        }
        other => panic!("expected a submodule diff, got {other:?}"),
    }
}

/// The parent's object database never holds a submodule's commits. Reading the gitlink in
/// the index as a blob asked it for one anyway: `Internal error: … could not be found`
/// on every click on a submodule marked `M` in Files (doc/12-risks.md, R-179).
#[test]
fn a_submodule_moved_in_the_working_tree_shows_both_commits() {
    let f = test_fixtures::with_submodule().unwrap();
    let recorded = f.oid("HEAD:vendor/lib").unwrap();
    f.git(&["-C", "vendor/lib", "checkout", "-q", "HEAD~1"])
        .unwrap();
    let moved = f.git(&["-C", "vendor/lib", "rev-parse", "HEAD"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let shown = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "vendor/lib",
            &DiffOptions::default(),
        )
        .expect("a moved submodule is a pointer change, not an internal error");

    match shown {
        FileDiff::Submodule {
            recorded: now,
            previous,
            ..
        } => {
            assert_eq!(now, moved.trim());
            assert_eq!(previous.as_deref(), Some(recorded.as_str()));
        }
        other => panic!("expected a submodule diff, got {other:?}"),
    }
}

#[test]
fn a_staged_submodule_pointer_shows_both_commits() {
    let f = test_fixtures::with_submodule().unwrap();
    let recorded = f.oid("HEAD:vendor/lib").unwrap();
    f.git(&["-C", "vendor/lib", "checkout", "-q", "HEAD~1"])
        .unwrap();
    f.git(&["add", "--", "vendor/lib"]).unwrap();
    let staged = f.oid(":vendor/lib").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let shown = state
        .diff_file(
            repo,
            &DiffSpec::IndexVsHead,
            "vendor/lib",
            &DiffOptions::default(),
        )
        .expect("a staged pointer is a pointer change, not an internal error");

    match shown {
        FileDiff::Submodule {
            recorded: now,
            previous,
            ..
        } => {
            assert_eq!(now, staged);
            assert_eq!(previous.as_deref(), Some(recorded.as_str()));
        }
        other => panic!("expected a submodule diff, got {other:?}"),
    }
}

/// Opens `path` so that nobody else may read it while the guard lives: whatever reads the
/// file's bytes fails, and only a look at its size still works.
#[cfg(windows)]
fn unreadable(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    let exclusive = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path);
    Some(exclusive.unwrap())
}

#[cfg(unix)]
fn unreadable(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
    None
}

// A multi-gigabyte file in the working tree was read whole — memory grew by its size —
// before its size said it would only be summarised.
#[test]
fn a_file_too_large_to_show_is_summarised_without_being_read() {
    let f = test_fixtures::linear(1).unwrap();
    let big = f.path().join("dump.bin");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(32 * 1024 * 1024)
        .unwrap();
    let _guard = unreadable(&big);
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "dump.bin",
            &DiffOptions::default(),
        )
        .unwrap();

    assert!(
        matches!(diff, FileDiff::TooLarge { size } if size == 32 * 1024 * 1024),
        "{diff:?}"
    );
}
