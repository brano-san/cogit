// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileStatus, RepoHandle, WorktreeFiles};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn paths(files: &[git_engine::FileEntry]) -> Vec<&str> {
    files.iter().map(|f| f.path.as_str()).collect()
}

fn status_of<'a>(files: &'a WorktreeFiles, section: &str, path: &str) -> &'a FileStatus {
    let list = if section == "staged" {
        &files.staged
    } else {
        &files.unstaged
    };
    &list
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("{path} is not in {section}: {:?}", paths(list)))
        .status
}

#[test]
fn a_clean_repository_has_nothing_to_show() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert!(files.staged.is_empty());
    assert!(files.unstaged.is_empty());
}

#[test]
fn a_staged_addition_lands_in_the_staged_section() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(paths(&files.staged), ["fresh.txt"]);
    assert_eq!(*status_of(&files, "staged", "fresh.txt"), FileStatus::Added);
    assert!(files.unstaged.is_empty());
}

#[test]
fn an_unstaged_edit_lands_in_the_unstaged_section() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "changed\n").unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert!(files.staged.is_empty());
    assert_eq!(
        *status_of(&files, "unstaged", "file0.txt"),
        FileStatus::Modified
    );
}

#[test]
fn a_file_edited_after_staging_appears_in_both_sections() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "staged\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    std::fs::write(f.path().join("file0.txt"), "staged then edited again\n").unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(paths(&files.staged), ["file0.txt"]);
    assert_eq!(paths(&files.unstaged), ["file0.txt"]);
}

#[test]
fn an_untracked_file_is_marked_untracked() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("scratch.txt"), "not added\n").unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(
        *status_of(&files, "unstaged", "scratch.txt"),
        FileStatus::Untracked
    );
}

#[test]
fn a_deletion_is_reported_on_the_side_it_happened() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(
        *status_of(&files, "unstaged", "file0.txt"),
        FileStatus::Deleted
    );
}

#[test]
fn a_staged_deletion_is_reported_as_staged() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["rm", "--", "file0.txt"]).unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(
        *status_of(&files, "staged", "file0.txt"),
        FileStatus::Deleted
    );
}

#[test]
fn a_conflicted_file_is_marked_conflicted() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert!(
        files
            .unstaged
            .iter()
            .any(|e| e.status == FileStatus::Conflicted),
        "an interrupted merge must surface its conflicts: {:?}",
        files
    );
}

#[test]
fn nested_paths_use_forward_slashes() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/new.rs"), "fn main() {}\n").unwrap();
    f.git(&["add", "--", "src/deep/new.rs"]).unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(paths(&files.staged), ["src/deep/new.rs"]);
}

#[test]
fn a_bare_repository_has_no_worktree_files() {
    let f = test_fixtures::bare().unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert!(files.staged.is_empty());
    assert!(files.unstaged.is_empty());
}

#[test]
fn each_section_is_sorted_by_path() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["zebra.txt", "alpha.txt", "middle.txt"] {
        std::fs::write(f.path().join(name), "x\n").unwrap();
    }
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();
    let listed = paths(&files.unstaged);

    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted);
}
