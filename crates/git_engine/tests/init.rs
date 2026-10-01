//! The Welcome dialog: what a picked folder is, and `git init` for one that is not a
//! repository.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FolderKind, GitError, RepoHandle, folder_kind, init_repository};

#[test]
fn a_folder_is_told_apart_from_a_repository_a_file_and_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let plain = parent.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    let file = parent.path().join("a.txt");
    std::fs::write(&file, "a").unwrap();

    assert_eq!(folder_kind(&plain), FolderKind::Plain);
    assert_eq!(folder_kind(&file), FolderKind::File);
    assert_eq!(
        folder_kind(&parent.path().join("gone")),
        FolderKind::Missing
    );

    let repo = test_fixtures::linear(1).unwrap();
    assert_eq!(folder_kind(repo.path()), FolderKind::Repository);
}

#[test]
fn init_makes_a_repository_that_opens_at_that_folder() {
    let parent = tempfile::tempdir().unwrap();
    let folder = parent.path().join("fresh");
    std::fs::create_dir(&folder).unwrap();

    let root = init_repository(&folder, None).unwrap();

    assert_eq!(root, folder);
    assert_eq!(folder_kind(&folder), FolderKind::Repository);
    RepoHandle::open_root(&folder).unwrap();
}

#[test]
fn init_creates_a_folder_that_is_missing() {
    let parent = tempfile::tempdir().unwrap();
    let folder = parent.path().join("new").join("deeper");

    init_repository(&folder, None).unwrap();

    assert_eq!(folder_kind(&folder), FolderKind::Repository);
}

#[test]
fn init_on_a_file_fails_with_gits_own_words() {
    let parent = tempfile::tempdir().unwrap();
    let file = parent.path().join("a.txt");
    std::fs::write(&file, "a").unwrap();

    let err = init_repository(&file, None).unwrap_err();

    let GitError::Command(failure) = err else {
        panic!("expected a command failure, got {err:?}");
    };
    assert_ne!(failure.exit_code, Some(0));
    assert!(!failure.stderr.is_empty());
}
