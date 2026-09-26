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
    let block = "alpha_step\nbeta_step\ngamma_step\n";
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
    let block = "alpha_step\nbeta_step\ngamma_step\n";
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

// `.gitattributes` was never read for a diff: a file marked `binary` or `-diff` showed as
// text and offered line staging, where git shows "Binary files differ".
#[test]
fn a_file_the_attributes_mark_binary_is_summarised_as_binary() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.dat binary\n*.lock -diff\n")
        .unwrap();
    f.write_file("table.dat", "one\n").unwrap();
    f.write_file("deps.lock", "one\n").unwrap();
    f.git(&["add", "--", ".gitattributes", "table.dat", "deps.lock"])
        .unwrap();
    f.commit_staged(1, "attributes").unwrap();
    f.write_file("table.dat", "two\n").unwrap();
    f.write_file("deps.lock", "two\n").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    for path in ["table.dat", "deps.lock"] {
        let diff = state
            .diff_file(
                repo,
                &DiffSpec::WorkTreeVsIndex,
                path,
                &DiffOptions::default(),
            )
            .unwrap();
        assert!(matches!(diff, FileDiff::Binary { .. }), "{path}: {diff:?}");
    }
}

#[test]
fn a_batch_of_files_honours_the_attributes_too() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.dat binary\n").unwrap();
    f.write_file("table.dat", "one\n").unwrap();
    f.git(&["add", "--", ".gitattributes", "table.dat"])
        .unwrap();
    f.commit_staged(1, "attributes").unwrap();
    f.write_file("table.dat", "two\n").unwrap();
    f.git(&["add", "--", "table.dat"]).unwrap();
    f.commit_staged(2, "change").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let batch = state
        .diff_files(
            repo,
            &head_vs_parent(&f),
            &["table.dat".to_owned()],
            &DiffOptions::default(),
            1,
        )
        .unwrap();

    let app_state::DiffBatch::Ready { files } = batch else {
        panic!("expected the batch to be ready");
    };
    assert!(
        matches!(files[0].diff, FileDiff::Binary { .. }),
        "{files:?}"
    );
}

// Normal states of a working tree ended in an error or in something untrue: a folder Git
// sees as one untracked entry was "absent from both sides of the diff", and a repository
// cloned inside this one was called a submodule.
#[test]
fn an_untracked_folder_is_described_rather_than_refused() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("generated/a.txt", "a\n").unwrap();
    f.write_file("generated/b.txt", "b\n").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let shown = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "generated/",
            &DiffOptions::default(),
        )
        .expect("an untracked folder is a normal state, not an error");

    assert!(
        matches!(shown, FileDiff::Folder { repository: false }),
        "{shown:?}"
    );
}

#[test]
fn a_repository_nested_inside_is_not_called_a_submodule() {
    let f = test_fixtures::linear(1).unwrap();
    let nested = f.path().join("vendor/x");
    std::fs::create_dir_all(&nested).unwrap();
    f.git_in(&nested, &["init", "-q"]).unwrap();
    std::fs::write(nested.join("n.txt"), "n\n").unwrap();
    f.git_in(&nested, &["add", "n.txt"]).unwrap();
    f.git_in(
        &nested,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "n",
        ],
    )
    .unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let shown = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "vendor/x",
            &DiffOptions::default(),
        )
        .unwrap();

    assert!(
        matches!(shown, FileDiff::Folder { repository: true }),
        "{shown:?}"
    );
}

fn renamed_with_one_edit(f: &test_fixtures::Fixture) {
    let body = (0..20).map(|i| format!("line {i}\n")).collect::<String>();
    f.commit_file(1, "a.txt", &body).unwrap();
    f.git(&["mv", "a.txt", "b.txt"]).unwrap();
    f.write_file("b.txt", &body.replace("line 7\n", "line seven\n"))
        .unwrap();
    f.git(&["add", "--", "b.txt"]).unwrap();
}

fn changed_rows(diff: &FileDiff) -> (usize, usize) {
    let FileDiff::Text { hunks, .. } = diff else {
        panic!("expected a text diff, got {diff:?}");
    };
    let rows = hunks.iter().flat_map(|hunk| &hunk.rows);
    rows.fold((0, 0), |(deleted, inserted), row| match row {
        diff_engine::DiffRow::Delete { .. } => (deleted + 1, inserted),
        diff_engine::DiffRow::Insert { .. } => (deleted, inserted + 1),
        _ => (deleted, inserted),
    })
}

#[test]
fn a_renamed_file_is_diffed_against_its_old_name() {
    let f = test_fixtures::linear(1).unwrap();
    renamed_with_one_edit(&f);
    f.commit_staged(2, "rename a.txt to b.txt").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(repo, &head_vs_parent(&f), "b.txt", &DiffOptions::default())
        .unwrap();

    assert_eq!(changed_rows(&diff), (1, 1));
}

#[test]
fn a_staged_rename_is_diffed_against_its_old_name() {
    let f = test_fixtures::linear(1).unwrap();
    renamed_with_one_edit(&f);
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let diff = state
        .diff_file(
            repo,
            &DiffSpec::IndexVsHead,
            "b.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    assert_eq!(changed_rows(&diff), (1, 1));
}
