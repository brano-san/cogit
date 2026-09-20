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

#[test]
fn an_untracked_directory_is_one_row_not_one_per_file() {
    let f = test_fixtures::linear(1).unwrap();
    let junk = f.path().join("generated");
    std::fs::create_dir_all(junk.join("deep")).unwrap();
    for i in 0..200 {
        std::fs::write(junk.join(format!("file-{i}.txt")), "x").unwrap();
        std::fs::write(junk.join("deep").join(format!("file-{i}.txt")), "x").unwrap();
    }
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(
        files.unstaged.len(),
        1,
        "400 files in one untracked directory must collapse: {:?}",
        paths(&files.unstaged)
    );
    assert_eq!(files.unstaged[0].path, "generated/");
}

#[test]
fn a_collapsed_directory_is_still_marked_untracked() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("build")).unwrap();
    std::fs::write(f.path().join("build/out.o"), "x").unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert_eq!(files.unstaged[0].status, FileStatus::Untracked);
}

#[test]
fn a_single_untracked_file_is_not_collapsed_into_a_directory() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("loose.txt"), "x").unwrap();
    let repo = open(&f);

    assert_eq!(
        paths(&repo.worktree_files().unwrap().unstaged),
        ["loose.txt"]
    );
}

#[test]
fn tracked_changes_inside_a_directory_are_never_collapsed() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src")).unwrap();
    std::fs::write(f.path().join("src/kept.txt"), "one\n").unwrap();
    f.git(&["add", "--", "src/kept.txt"]).unwrap();
    f.commit_staged(1, "add src/kept.txt").unwrap();
    std::fs::write(f.path().join("src/kept.txt"), "two\n").unwrap();
    std::fs::write(f.path().join("src/new.txt"), "x").unwrap();
    let repo = open(&f);

    let files = repo.worktree_files().unwrap();

    assert!(
        paths(&files.unstaged).contains(&"src/kept.txt"),
        "a tracked edit must stay visible: {:?}",
        paths(&files.unstaged)
    );
}

#[test]
fn ignoring_a_path_writes_it_to_gitignore() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("secret.env"), "TOKEN=1\n").unwrap();
    let repo = open(&f);

    repo.add_to_gitignore(&["secret.env".to_owned()]).unwrap();

    let ignore = std::fs::read_to_string(f.path().join(".gitignore")).unwrap();
    assert!(ignore.contains("secret.env"), "{ignore:?}");
    assert!(
        !repo
            .worktree_files()
            .unwrap()
            .unstaged
            .iter()
            .any(|e| e.path == "secret.env"),
        "an ignored file leaves the list"
    );
}

#[test]
fn ignoring_appends_without_losing_what_was_there() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitignore", "already-here\n").unwrap();
    let repo = open(&f);

    repo.add_to_gitignore(&["and-this".to_owned()]).unwrap();

    let ignore = std::fs::read_to_string(f.path().join(".gitignore")).unwrap();
    assert!(ignore.contains("already-here"));
    assert!(ignore.contains("and-this"));
}

#[test]
fn ignoring_the_same_path_twice_does_not_duplicate_it() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    repo.add_to_gitignore(&["once".to_owned()]).unwrap();

    repo.add_to_gitignore(&["once".to_owned()]).unwrap();

    let ignore = std::fs::read_to_string(f.path().join(".gitignore")).unwrap();
    assert_eq!(ignore.matches("once").count(), 1, "{ignore:?}");
}

#[test]
fn an_empty_ignore_list_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).add_to_gitignore(&[]).is_err());
}

#[test]
fn deleting_an_untracked_file_removes_it_from_disk() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("junk.txt"), "x").unwrap();
    let repo = open(&f);

    repo.delete_untracked(&["junk.txt".to_owned()]).unwrap();

    assert!(!f.path().join("junk.txt").exists());
}

#[test]
fn deleting_refuses_a_tracked_file() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.delete_untracked(&["file0.txt".to_owned()]).is_err());
    assert!(
        f.path().join("file0.txt").exists(),
        "a tracked file must survive a refused delete"
    );
}
