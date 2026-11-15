// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

/// A stash of every kind of change: two edits (one staged), a deletion, a staged new file
/// and an untracked one (`^3`).
fn stashed() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("file0.txt", "stashed 0\n").unwrap();
    f.write_file("file1.txt", "stashed 1\n").unwrap();
    f.git(&["add", "file1.txt"]).unwrap();
    std::fs::remove_file(f.path().join("file2.txt")).unwrap();
    f.write_file("added.txt", "added\n").unwrap();
    f.git(&["add", "added.txt"]).unwrap();
    f.write_file("fresh.txt", "fresh\n").unwrap();
    f.git_at(10, &["stash", "push", "-u", "-m", "wip"]).unwrap();
    f
}

fn oid(state: &AppState, repo: RepoId) -> String {
    state.stashes(repo).unwrap()[0].oid.clone()
}

fn paths(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn text(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

fn status(f: &test_fixtures::Fixture) -> String {
    f.git(&["status", "--porcelain", "--untracked-files=all"])
        .unwrap()
}

const EVERYTHING: [&str; 5] = [
    "file0.txt",
    "file1.txt",
    "file2.txt",
    "added.txt",
    "fresh.txt",
];

#[test]
fn only_the_chosen_tracked_files_are_applied() {
    let f = stashed();
    let (state, repo) = open(&f);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &paths(&["file0.txt"]), false)
        .unwrap();

    assert_eq!(text(&f, "file0.txt"), "stashed 0\n");
    assert_eq!(text(&f, "file1.txt"), "content 1\n");
    assert_eq!(status(&f), " M file0.txt\n");
    assert_eq!(state.stashes(repo).unwrap().len(), 1, "the stash stays");
}

#[test]
fn restoring_the_index_brings_a_staged_file_back_staged() {
    let f = stashed();
    let (state, repo) = open(&f);

    state
        .stash_apply_paths(
            repo,
            &oid(&state, repo),
            &paths(&["file0.txt", "file1.txt"]),
            true,
        )
        .unwrap();

    assert_eq!(status(&f), " M file0.txt\nM  file1.txt\n");
}

#[test]
fn an_untracked_file_comes_from_the_third_parent_untracked() {
    let f = stashed();
    let (state, repo) = open(&f);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &paths(&["fresh.txt"]), false)
        .unwrap();

    assert_eq!(text(&f, "fresh.txt"), "fresh\n");
    assert_eq!(status(&f), "?? fresh.txt\n");
}

#[test]
fn a_file_deleted_in_the_stash_is_deleted() {
    let f = stashed();
    let (state, repo) = open(&f);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &paths(&["file2.txt"]), false)
        .unwrap();

    assert!(!f.path().join("file2.txt").exists());
    assert_eq!(status(&f), " D file2.txt\n");
}

#[test]
fn a_chosen_file_with_local_changes_refuses_and_writes_nothing() {
    let f = stashed();
    let (state, repo) = open(&f);
    f.write_file("file1.txt", "local edit\n").unwrap();

    let err = state
        .stash_apply_paths(
            repo,
            &oid(&state, repo),
            &paths(&["file0.txt", "file1.txt"]),
            false,
        )
        .unwrap_err();

    assert!(err.to_string().contains("file1.txt"), "{err}");
    assert_eq!(text(&f, "file0.txt"), "content 0\n");
    assert_eq!(text(&f, "file1.txt"), "local edit\n");
    assert!(state.safety_log().is_empty());
}

#[test]
fn an_untracked_file_in_the_way_refuses_and_writes_nothing() {
    let f = stashed();
    let (state, repo) = open(&f);
    f.write_file("fresh.txt", "mine\n").unwrap();

    let err = state
        .stash_apply_paths(
            repo,
            &oid(&state, repo),
            &paths(&["file0.txt", "fresh.txt"]),
            false,
        )
        .unwrap_err();

    assert!(err.to_string().contains("fresh.txt"), "{err}");
    assert_eq!(text(&f, "file0.txt"), "content 0\n");
    assert_eq!(text(&f, "fresh.txt"), "mine\n");
}

#[test]
fn a_path_the_stash_does_not_hold_is_refused() {
    let f = stashed();
    let (state, repo) = open(&f);

    assert!(
        state
            .stash_apply_paths(repo, &oid(&state, repo), &paths(&["nowhere.txt"]), false)
            .is_err()
    );
    assert_eq!(status(&f), "");
}

#[test]
fn undo_puts_back_what_the_partial_apply_overwrote_and_removes_what_it_created() {
    let f = stashed();
    let (state, repo) = open(&f);
    let chosen = paths(&[
        "file0.txt",
        "file1.txt",
        "file2.txt",
        "added.txt",
        "fresh.txt",
    ]);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &chosen, true)
        .unwrap();
    assert_ne!(status(&f), "");
    state.undo_last(repo).unwrap();

    assert_eq!(status(&f), "");
    assert_eq!(text(&f, "file0.txt"), "content 0\n");
    assert_eq!(text(&f, "file1.txt"), "content 1\n");
    assert_eq!(text(&f, "file2.txt"), "content 2\n");
    assert!(!f.path().join("added.txt").exists());
    assert!(!f.path().join("fresh.txt").exists());
}

#[test]
fn undo_refuses_once_an_applied_file_changed_since() {
    let f = stashed();
    let (state, repo) = open(&f);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &paths(&["file0.txt"]), false)
        .unwrap();
    f.write_file("file0.txt", "edited after\n").unwrap();

    assert!(state.undo_last(repo).is_err());
    assert_eq!(text(&f, "file0.txt"), "edited after\n");
}

fn same_as_plain_apply(restore_index: bool) {
    let partial = stashed();
    let plain = stashed();
    let (state, repo) = open(&partial);
    let (other, other_repo) = open(&plain);

    state
        .stash_apply_paths(repo, &oid(&state, repo), &paths(&EVERYTHING), restore_index)
        .unwrap();
    other
        .stash_apply(other_repo, &oid(&other, other_repo), false, restore_index)
        .unwrap();

    assert_eq!(status(&partial), status(&plain));
    for name in EVERYTHING {
        assert_eq!(
            partial.path().join(name).exists(),
            plain.path().join(name).exists(),
            "{name}"
        );
        if plain.path().join(name).exists() {
            assert_eq!(text(&partial, name), text(&plain, name), "{name}");
        }
    }
}

#[test]
fn every_file_chosen_ends_as_a_plain_apply_does() {
    same_as_plain_apply(false);
}

#[test]
fn every_file_chosen_with_the_index_ends_as_apply_index_does() {
    same_as_plain_apply(true);
}
