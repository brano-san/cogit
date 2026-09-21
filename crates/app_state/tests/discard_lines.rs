// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Discarding a line range: the destructive half of gutter staging (T6.7).

use app_state::AppState;
use diff_engine::{DiffOptions, FileDiff, LineEnding, PatchRequest, diff_text};

/// Two lines added on top of the committed content.
fn two_added_lines() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "content 0\nfirst added\nsecond added\n",
    )
    .unwrap();
    f
}

fn request(path: &str, inserts: Vec<u32>) -> PatchRequest {
    let diff = diff_text(
        "content 0\n",
        "content 0\nfirst added\nsecond added\n",
        &DiffOptions::default(),
    );
    let FileDiff::Text { hunks, .. } = diff else {
        panic!("expected a text diff");
    };
    PatchRequest {
        path: path.to_owned(),
        hunks,
        selected_deletes: Vec::new(),
        selected_inserts: inserts,
        line_ending: LineEnding::Lf,
        no_trailing_newline: false,
    }
}

#[test]
fn discarding_a_line_takes_it_out_of_the_file() {
    let f = two_added_lines();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .discard_selection(repo, &request("file0.txt", vec![2]))
        .unwrap();

    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert_eq!(text, "content 0\nsecond added\n");
}

#[test]
fn the_lines_that_were_not_selected_stay() {
    let f = two_added_lines();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .discard_selection(repo, &request("file0.txt", vec![2]))
        .unwrap();

    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert!(text.contains("second added"), "{text}");
}

#[test]
fn a_discard_is_written_to_the_safety_journal() {
    let f = two_added_lines();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .discard_selection(repo, &request("file0.txt", vec![2]))
        .unwrap();

    let entries = state.safety_log();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].description.contains("file0.txt"), "{entries:?}");
    assert!(
        !entries[0].undoable,
        "a line-level discard has nowhere to restore from, and says so"
    );
}

#[test]
fn selecting_nothing_is_refused_rather_than_discarding_everything() {
    let f = two_added_lines();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let failed = state.discard_selection(repo, &request("file0.txt", Vec::new()));

    assert!(failed.is_err());
    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert!(text.contains("first added"), "nothing may be thrown away");
}

#[test]
fn discarding_does_not_stage_anything() {
    let f = two_added_lines();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .discard_selection(repo, &request("file0.txt", vec![2]))
        .unwrap();

    assert!(
        state
            .worktree_files(repo, git_engine::WorktreeView::default())
            .unwrap()
            .staged
            .is_empty()
    );
}
