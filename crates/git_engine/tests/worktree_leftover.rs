#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A folder git failed to delete after it dropped the worktree's record (R-184): it may be
//! removed by hand, but never while it is a worktree, a repository, or around one.

use git_engine::RepoHandle;

/// The state git 2.51 leaves when a lock keeps `worktree remove --force` from finishing:
/// the record is gone, the folder and its files are not.
fn orphaned() -> (test_fixtures::Fixture, RepoHandle, String) {
    let f = test_fixtures::with_worktree().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let linked = repo.worktrees().unwrap().pop().unwrap();
    std::fs::write(format!("{}/kept.txt", linked.path), "x").unwrap();
    let records = f.path().join(".git/worktrees");
    std::fs::remove_dir_all(records).unwrap();
    (f, repo, linked.path)
}

#[test]
fn an_unregistered_folder_is_reported_and_deleted() {
    let (_f, repo, path) = orphaned();

    assert!(repo.worktree_leftover(&path));
    repo.delete_worktree_leftover(&path).unwrap();

    assert!(!std::path::Path::new(&path).exists());
    assert!(!repo.worktree_leftover(&path));
}

#[test]
fn a_registered_worktree_is_not_a_leftover() {
    let f = test_fixtures::with_worktree().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let path = repo.worktrees().unwrap().pop().unwrap().path;

    assert!(!repo.worktree_leftover(&path));
    assert!(repo.delete_worktree_leftover(&path).is_err());
    assert!(std::path::Path::new(&path).exists());
}

#[test]
fn the_repository_itself_and_its_parents_are_never_deleted() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let root = f
        .path()
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let parent = f
        .path()
        .parent()
        .unwrap()
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");

    for path in [root, parent] {
        assert!(!repo.worktree_leftover(&path), "{path}");
        assert!(repo.delete_worktree_leftover(&path).is_err(), "{path}");
    }
    assert!(f.path().join(".git").exists());
}

#[test]
fn a_folder_holding_a_repository_is_kept() {
    let (f, repo, _path) = orphaned();
    let nested = f.path().join("nested");
    std::fs::create_dir_all(nested.join(".git")).unwrap();
    let nested = nested
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");

    assert!(repo.delete_worktree_leftover(&nested).is_err());
    assert!(std::path::Path::new(&nested).exists());
}
