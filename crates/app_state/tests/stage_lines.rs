// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Line staging end to end. `diff_engine` already proves it builds the right patch text
//! and `git_engine` already proves `git apply --cached` runs; nothing until now fed one
//! to the other, which is where the envelope for a created or deleted file gets decided.

use app_state::AppState;
use diff_engine::{DiffOptions, FileDiff, LineEnding, PatchRequest, diff_text};

fn hunks_of(old: &str, new: &str) -> Vec<diff_engine::Hunk> {
    match diff_text(old, new, &DiffOptions::default()) {
        FileDiff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn request(path: &str, old: &str, new: &str, deletes: Vec<u32>, inserts: Vec<u32>) -> PatchRequest {
    PatchRequest {
        path: path.to_owned(),
        hunks: hunks_of(old, new),
        selected_deletes: deletes,
        selected_inserts: inserts,
        line_ending: LineEnding::Lf,
        no_trailing_newline: false,
    }
}

/// Read back through a second handle: the assertion is about what git holds, not about
/// what `AppState` remembers.
fn staged_text(root: &std::path::Path, path: &str) -> String {
    let (_, after) = git_engine::RepoHandle::open(root)
        .unwrap()
        .diff_sides(&git_engine::DiffSpec::IndexVsHead, path)
        .unwrap();
    String::from_utf8(after.unwrap_or_default()).unwrap()
}

#[test]
fn staging_only_a_deletion_leaves_the_addition_in_the_same_hunk_unstaged() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "mixed.txt", "keep\nremove me\ntail\n")
        .unwrap();
    f.write_file("mixed.txt", "keep\nadded\ntail\n").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let old = "keep\nremove me\ntail\n";
    let new = "keep\nadded\ntail\n";

    state
        .stage_selection(
            repo,
            &request("mixed.txt", old, new, vec![2], Vec::new()),
            false,
        )
        .unwrap();

    assert_eq!(
        staged_text(f.path(), "mixed.txt"),
        "keep\ntail\n",
        "only the removed line may reach the index"
    );
    assert!(
        std::fs::read_to_string(f.path().join("mixed.txt"))
            .unwrap()
            .contains("added"),
        "the working tree must keep the addition"
    );
}

#[test]
fn staging_a_line_of_a_file_that_has_no_committed_version_creates_it_in_the_index() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("fresh.txt", "one\ntwo\n").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .stage_selection(
            repo,
            &request("fresh.txt", "", "one\ntwo\n", Vec::new(), vec![1]),
            false,
        )
        .unwrap();

    assert_eq!(
        staged_text(f.path(), "fresh.txt"),
        "one\n",
        "the index gets a new file holding just the picked line"
    );
}

#[test]
fn staging_the_removal_of_every_line_records_the_file_as_deleted() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "gone.txt", "alpha\nbeta\n").unwrap();
    std::fs::remove_file(f.path().join("gone.txt")).unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .stage_selection(
            repo,
            &request("gone.txt", "alpha\nbeta\n", "", vec![1, 2], Vec::new()),
            false,
        )
        .unwrap();

    let files = state
        .worktree_files(repo, git_engine::WorktreeView::default())
        .unwrap();
    assert!(
        files.staged.iter().any(|entry| entry.path == "gone.txt"),
        "the deletion must be staged: {:?}",
        files.staged
    );
}
