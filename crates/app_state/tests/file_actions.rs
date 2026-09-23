#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Files panel's context menu through `AppState`: the Index Editor saves only the
//! sides that were changed, and Remove leaves a line in the safety journal.

use app_state::AppState;

#[test]
fn the_index_editor_writes_only_the_sides_it_was_given() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "on disk\n").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .write_index_editor(repo, "file0.txt", Some("staged by hand\n"), None)
        .unwrap();

    let sides = state.index_editor_sides(repo, "file0.txt").unwrap();
    assert_eq!(sides.index.as_deref(), Some("staged by hand\n"));
    assert_eq!(sides.worktree.as_deref(), Some("on disk\n"));

    state
        .write_index_editor(repo, "file0.txt", None, Some("typed\n"))
        .unwrap();
    let sides = state.index_editor_sides(repo, "file0.txt").unwrap();
    assert_eq!(sides.index.as_deref(), Some("staged by hand\n"));
    assert_eq!(sides.worktree.as_deref(), Some("typed\n"));
}

#[test]
fn removing_from_the_repository_is_journalled() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .remove_from_repository(repo, &["file0.txt".to_owned()], true)
        .unwrap();

    assert!(!f.path().join("file0.txt").exists());
    let journal = state.safety_log();
    assert!(
        journal
            .iter()
            .any(|entry| entry.description.contains("file0.txt")),
        "{journal:?}"
    );
}

#[test]
fn a_read_only_copy_lands_in_the_temporary_folder() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let copy = state.export_read_only(repo, "HEAD", "file0.txt").unwrap();

    assert!(copy.starts_with(std::env::temp_dir()), "{}", copy.display());
    assert_eq!(std::fs::read_to_string(copy).unwrap(), "content 0\n");
}

#[test]
fn the_root_of_an_open_repository_is_known_and_a_closed_one_is_not() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let root = state.root_of(repo).unwrap();
    assert!(root.join("file0.txt").exists());

    state.close_repository(repo);
    assert!(state.root_of(repo).is_err());
}
