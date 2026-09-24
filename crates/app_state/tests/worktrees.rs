// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Worktrees as a panel of their own: opened like a submodule, removed without losing
//! work (doc/12-risks.md, R-184).

use app_state::{AppState, RepoId};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn linked(state: &AppState, repo: RepoId) -> git_engine::WorktreeEntry {
    state
        .worktrees(repo)
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap()
}

#[test]
fn a_worktree_opens_in_the_panels_without_joining_the_repository_list() {
    let f = test_fixtures::with_worktree().unwrap();
    let (state, owner) = open(&f);
    let path = linked(&state, owner).path;

    let opened = state.open_worktree(owner, &path).unwrap();

    assert_eq!(opened.root.replace('\\', "/"), path, "{opened:?}");
    assert_ne!(opened.repo, owner);
    assert_eq!(state.overviews().len(), 1, "the list is not rebuilt");
    let current: Vec<bool> = state
        .worktrees(opened.repo)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.path == path)
        .map(|entry| entry.is_current)
        .collect();
    assert_eq!(current, [true]);
}

#[test]
fn a_folder_that_is_not_one_of_its_worktrees_is_not_opened_as_one() {
    let f = test_fixtures::with_worktree().unwrap();
    let other = test_fixtures::linear(1).unwrap();
    let (state, owner) = open(&f);

    let refused = state.open_worktree(owner, &other.path().to_string_lossy().replace('\\', "/"));

    assert!(
        matches!(refused, Err(git_engine::GitError::InvalidState(_))),
        "{refused:?}"
    );
}

#[test]
fn a_missing_worktree_cannot_be_opened() {
    let f = test_fixtures::with_worktree().unwrap();
    let (state, owner) = open(&f);
    let path = linked(&state, owner).path;
    std::fs::remove_dir_all(&path).unwrap();

    assert!(state.open_worktree(owner, &path).is_err());
}

/// `--force` would throw the changes away; they go into a stash first, and Undo in the
/// journal applies it (INV-12).
#[test]
fn a_dirty_worktree_removed_by_force_leaves_its_changes_in_a_stash() {
    let f = test_fixtures::with_worktree().unwrap();
    let (state, owner) = open(&f);
    let path = linked(&state, owner).path;
    std::fs::write(std::path::Path::new(&path).join("file0.txt"), "work\n").unwrap();

    state.remove_worktree(owner, &path, true).unwrap();

    assert_eq!(state.worktrees(owner).unwrap().len(), 1);
    let stashes = state.stashes(owner).unwrap();
    assert!(
        stashes
            .iter()
            .any(|entry| entry.message.contains("before removing worktree linked")),
        "{stashes:?}"
    );
    let journal = state.safety_log();
    assert!(journal[0].description.contains("linked"), "{journal:?}");
    assert!(journal[0].undoable, "{journal:?}");
}

#[test]
fn a_clean_worktree_is_removed_without_a_stash() {
    let f = test_fixtures::with_worktree().unwrap();
    let (state, owner) = open(&f);
    let path = linked(&state, owner).path;

    state.remove_worktree(owner, &path, false).unwrap();

    assert!(state.stashes(owner).unwrap().is_empty());
    assert!(!std::path::Path::new(&path).exists());
}

// Undo applied the stash of the removed worktree in the owner's working tree, on another
// branch: the feature's changes landed in main's files, and the worktree stayed gone.
#[test]
fn undoing_a_forced_removal_brings_the_worktree_back_with_its_changes() {
    let f = test_fixtures::with_worktree().unwrap();
    let (state, owner) = open(&f);
    let path = linked(&state, owner).path;
    std::fs::write(std::path::Path::new(&path).join("file0.txt"), "work\n").unwrap();
    let main_before = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    state.remove_worktree(owner, &path, true).unwrap();

    state.undo_last(owner).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        main_before
    );
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&path).join("file0.txt")).unwrap(),
        "work\n"
    );
    assert_eq!(state.worktrees(owner).unwrap().len(), 2);
}
