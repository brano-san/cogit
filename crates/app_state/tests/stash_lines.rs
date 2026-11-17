#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use diff_engine::{DiffOptions, FileDiff, PatchRequest, diff_text};

const COMMITTED: &str = "content 0\n";
const EDITED: &str = "content 0\nfirst added\nsecond added\n";

fn two_added_lines() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", EDITED).unwrap();
    f
}

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn request(inserts: Vec<u32>) -> PatchRequest {
    let FileDiff::Text { hunks, .. } = diff_text(COMMITTED, EDITED, &DiffOptions::default()) else {
        panic!("expected a text diff");
    };
    PatchRequest {
        path: "file0.txt".to_owned(),
        hunks,
        selected_deletes: Vec::new(),
        selected_inserts: inserts,
    }
}

fn text(f: &test_fixtures::Fixture) -> String {
    std::fs::read_to_string(f.path().join("file0.txt")).unwrap()
}

#[test]
fn the_chosen_line_goes_into_a_stash_and_the_other_stays() {
    let f = two_added_lines();
    let (state, repo) = open(&f);

    state
        .stash_hunks(repo, &[request(vec![2])], "one line")
        .unwrap();

    assert_eq!(text(&f), "content 0\nsecond added\n");
    let stashes = state.stashes(repo).unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(
        f.git(&["show", &format!("{}:file0.txt", stashes[0].oid)])
            .unwrap(),
        "content 0\nfirst added\n"
    );
}

#[test]
fn undo_puts_the_line_back_and_drops_the_stash() {
    let f = two_added_lines();
    let (state, repo) = open(&f);
    state.stash_hunks(repo, &[request(vec![2])], "").unwrap();
    let entry = state.safety_log();
    assert!(entry[0].undoable, "{entry:?}");
    assert!(entry[0].description.contains("file0.txt"), "{entry:?}");

    state.undo_last(repo).unwrap();

    assert_eq!(text(&f), EDITED);
    assert!(state.stashes(repo).unwrap().is_empty());
}

#[test]
fn nothing_selected_is_refused() {
    let f = two_added_lines();
    let (state, repo) = open(&f);

    assert!(state.stash_hunks(repo, &[], "").is_err());
    assert!(state.stash_hunks(repo, &[request(Vec::new())], "").is_err());

    assert_eq!(text(&f), EDITED);
    assert!(state.stashes(repo).unwrap().is_empty());
}
