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

fn request_with(
    path: &str,
    old: &str,
    new: &str,
    options: &DiffOptions,
    deletes: Vec<u32>,
    inserts: Vec<u32>,
) -> PatchRequest {
    let hunks = match diff_text(old, new, options) {
        FileDiff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    };
    PatchRequest {
        hunks,
        ..request(path, old, new, deletes, inserts)
    }
}

fn opened(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn index_text(f: &test_fixtures::Fixture, path: &str) -> String {
    f.git(&["show", &format!(":{path}")]).unwrap()
}

// The patch was always built to be applied forward. Discard applies it backwards to the
// working tree, which already holds the unselected insertions and not the unselected
// deletions: with any other change in the same hunk, git said "patch does not apply".
#[test]
fn discarding_one_change_keeps_the_other_change_in_its_hunk() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "f.txt", "a\nb\nc\n").unwrap();
    f.write_file("f.txt", "A\nb\nC\n").unwrap();
    let (state, repo) = opened(&f);

    state
        .discard_selection(
            repo,
            &request("f.txt", "a\nb\nc\n", "A\nb\nC\n", vec![1], vec![1]),
        )
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("f.txt")).unwrap(),
        "a\nb\nC\n"
    );
}

#[test]
fn unstaging_one_change_keeps_the_other_change_in_its_hunk() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "f.txt", "a\nb\nc\n").unwrap();
    f.write_file("f.txt", "A\nb\nC\n").unwrap();
    f.git(&["add", "f.txt"]).unwrap();
    let (state, repo) = opened(&f);

    state
        .stage_selection(
            repo,
            &request("f.txt", "a\nb\nc\n", "A\nb\nC\n", vec![1], vec![1]),
            true,
        )
        .unwrap();

    assert_eq!(index_text(&f, "f.txt"), "a\nb\nC\n");
}

// With no context lines a hunk that only inserts has no old lines, and its header said
// `-0,0`: git put the line at the top of the file instead of after line 5.
#[test]
fn a_line_added_mid_file_is_staged_where_it_was_added_with_no_context() {
    let f = test_fixtures::empty().unwrap();
    let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    let new = "1\n2\n3\n4\n5\nX\n6\n7\n8\n9\n10\n";
    f.commit_file(1, "f.txt", old).unwrap();
    f.write_file("f.txt", new).unwrap();
    let (state, repo) = opened(&f);
    let options = DiffOptions {
        context_lines: 0,
        ..DiffOptions::default()
    };

    state
        .stage_selection(
            repo,
            &request_with("f.txt", old, new, &options, Vec::new(), vec![6]),
            false,
        )
        .unwrap();

    assert_eq!(index_text(&f, "f.txt"), new);
}

// One marker at the very end of the patch fits neither a hunk that ends on context nor
// a changed last line: git refused both.
#[test]
fn a_change_near_the_end_of_a_file_without_a_final_newline_stages() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "f.txt", "a\nb\nc").unwrap();
    f.write_file("f.txt", "A\nb\nc").unwrap();
    let (state, repo) = opened(&f);

    state
        .stage_selection(
            repo,
            &request("f.txt", "a\nb\nc", "A\nb\nc", vec![1], vec![1]),
            false,
        )
        .unwrap();

    assert_eq!(index_text(&f, "f.txt"), "A\nb\nc");
}

#[test]
fn a_changed_last_line_without_a_final_newline_stages() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "f.txt", "a\nb").unwrap();
    f.write_file("f.txt", "a\nB").unwrap();
    let (state, repo) = opened(&f);

    state
        .stage_selection(
            repo,
            &request("f.txt", "a\nb", "a\nB", vec![2], vec![2]),
            false,
        )
        .unwrap();

    assert_eq!(index_text(&f, "f.txt"), "a\nB");
}

// Part of a deleted file: the patch said `+++ /dev/null` with lines still in it, and git
// answered "deleted file still has contents".
#[test]
fn staging_part_of_a_deletion_keeps_the_rest_of_the_file() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "h.txt", "x\ny\nz\n").unwrap();
    std::fs::remove_file(f.path().join("h.txt")).unwrap();
    let (state, repo) = opened(&f);

    state
        .stage_selection(
            repo,
            &request("h.txt", "x\ny\nz\n", "", vec![1], Vec::new()),
            false,
        )
        .unwrap();

    assert_eq!(index_text(&f, "h.txt"), "y\nz\n");
}

#[test]
fn unstaging_part_of_a_new_file_keeps_the_rest_of_it() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("fresh.txt", "one\ntwo\n").unwrap();
    f.git(&["add", "fresh.txt"]).unwrap();
    let (state, repo) = opened(&f);

    state
        .stage_selection(
            repo,
            &request("fresh.txt", "", "one\ntwo\n", Vec::new(), vec![1]),
            true,
        )
        .unwrap();

    assert_eq!(index_text(&f, "fresh.txt"), "two\n");
}

// A line that is not UTF-8 reaches the viewer with its odd bytes replaced. Staged one line
// at a time, the replacement went into the index instead of the bytes: the file was
// changed silently. Line by line is refused there; the whole file still stages.
#[test]
fn a_line_that_is_not_utf8_is_not_staged_as_something_else() {
    let f = test_fixtures::empty().unwrap();
    f.commit_file(1, "latin.txt", "a\nb\n").unwrap();
    std::fs::write(f.path().join("latin.txt"), b"a\ncaf\xe9\n").unwrap();
    let (state, repo) = opened(&f);
    let hunks = match diff_engine::diff_bytes(b"a\nb\n", b"a\ncaf\xe9\n", &DiffOptions::default()) {
        FileDiff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    };
    let request = PatchRequest {
        hunks,
        ..request("latin.txt", "a\nb\n", "a\ncafe\n", vec![2], vec![2])
    };

    let result = state.stage_selection(repo, &request, false);

    assert!(result.is_err(), "{result:?}");
    assert_eq!(index_text(&f, "latin.txt"), "a\nb\n");
}
