// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, FileContent};
use git_engine::DiffSpec;

fn opened(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

// An unchanged file showed "No change in this file." instead of the file.
#[test]
fn an_unchanged_file_is_read_whole_from_the_side_shown() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("notes.md", "# Notes\n\none\n").unwrap();
    f.git(&["add", "."]).unwrap();
    f.commit_staged(5, "notes").unwrap();
    let (state, repo) = opened(&f);

    let content = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "notes.md", None)
        .unwrap();
    assert_eq!(
        content,
        FileContent::Text {
            text: "# Notes\n\none\n".to_owned()
        }
    );
}

#[test]
fn an_ignored_file_is_read_from_the_working_tree() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitignore", "*.log\n").unwrap();
    f.write_file("build.log", "done\n").unwrap();
    let (state, repo) = opened(&f);

    let content = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "build.log", None)
        .unwrap();
    assert_eq!(
        content,
        FileContent::Text {
            text: "done\n".to_owned()
        }
    );
}

#[test]
fn a_binary_file_is_named_by_its_size() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("blob.bin"), [0_u8, 1, 2, 3, 0, 5]).unwrap();
    let (state, repo) = opened(&f);

    let content = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "blob.bin", None)
        .unwrap();
    assert_eq!(content, FileContent::Binary { size: 6 });
}

#[test]
fn a_file_past_the_limit_waits_for_show_anyway() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("big.txt", &"line\n".repeat(10)).unwrap();
    let (state, repo) = opened(&f);

    let held = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "big.txt", Some(20))
        .unwrap();
    assert_eq!(
        held,
        FileContent::TooLarge {
            size: 50,
            limit: 20
        }
    );

    let shown = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "big.txt", Some(1000))
        .unwrap();
    assert!(matches!(shown, FileContent::Text { .. }), "{shown:?}");
}

#[test]
fn a_file_gone_from_disk_is_missing() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);

    let content = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "nowhere.txt", None)
        .unwrap();
    assert_eq!(content, FileContent::Missing);
}

#[test]
fn a_utf16_file_is_decoded() {
    let f = test_fixtures::linear(1).unwrap();
    let mut bytes = vec![0xFF, 0xFE];
    for unit in "wide\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(f.path().join("wide.txt"), bytes).unwrap();
    let (state, repo) = opened(&f);

    let content = state
        .file_content(repo, &DiffSpec::WorkTreeVsIndex, "wide.txt", None)
        .unwrap();
    assert_eq!(
        content,
        FileContent::Text {
            text: "wide\n".to_owned()
        }
    );
}
