// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn read(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

#[test]
fn keeping_the_working_tree_stashes_the_changes_and_leaves_the_files_alone() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "work in progress\n").unwrap();
    let repo = open(&f);

    repo.stash_keeping_worktree("parser, half done").unwrap();

    assert_eq!(read(&f, "file0.txt"), "work in progress\n");
    let stashes = repo.stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(
        stashes[0].message.contains("parser, half done"),
        "{:?}",
        stashes[0]
    );
}

#[test]
fn the_kept_stash_holds_the_change() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "work in progress\n").unwrap();
    let repo = open(&f);
    repo.stash_keeping_worktree("wip").unwrap();

    f.git(&["checkout", "--", "file0.txt"]).unwrap();
    f.git(&["stash", "apply"]).unwrap();

    assert_eq!(read(&f, "file0.txt"), "work in progress\n");
}

#[test]
fn keeping_the_working_tree_of_a_clean_tree_is_refused() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.stash_keeping_worktree("nothing").is_err());
    assert!(repo.stashes().unwrap().is_empty());
}

#[test]
fn a_selection_stash_without_a_message_gets_git_s_own() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "wip\n").unwrap();
    let repo = open(&f);

    repo.stash_paths(&["file0.txt".to_owned()], "").unwrap();

    let message = &repo.stashes().unwrap()[0].message;
    assert!(message.starts_with("WIP on "), "{message}");
}

/// Apply Stash (#30): a conflicted apply is a failed command, so its output — the
/// `CONFLICT` lines — reaches the notification window whole.
#[test]
fn applying_the_newest_stash_over_a_conflicting_commit_fails_with_git_s_output() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "stashed side\n").unwrap();
    let repo = open(&f);
    repo.stash_keeping_worktree("wip").unwrap();
    f.git(&["checkout", "--", "file0.txt"]).unwrap();
    f.commit_file(30, "file0.txt", "committed side\n").unwrap();

    let err = repo.stash_apply_index(0, false).unwrap_err();

    let git_engine::GitError::Command(failure) = err else {
        panic!("expected a command failure, got {err:?}");
    };
    let output = format!("{}{}", failure.stdout, failure.stderr);
    assert!(output.contains("CONFLICT"), "{output}");
}

/// `stash push` hands the paths to a `git add` of its own, which refuses one that is in
/// neither the index nor the folder: the stash was made and the edits stayed (R-486).
#[test]
fn a_selection_with_a_staged_deletion_stashes_the_rest() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["rm", "-q", "file0.txt"]).unwrap();
    std::fs::write(f.path().join("file1.txt"), "wip\n").unwrap();
    let repo = open(&f);

    let paths = ["file0.txt".to_owned(), "file1.txt".to_owned()];
    let stashed = repo.stash_paths(&paths, "both").unwrap();

    assert!(stashed.is_some());
    assert_eq!(repo.stashes().unwrap().len(), 1);
    assert_eq!(read(&f, "file1.txt"), "content 1\n");
}

#[test]
fn a_selection_of_nothing_but_a_staged_deletion_leaves_no_stash_behind() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["rm", "-q", "file0.txt"]).unwrap();
    let repo = open(&f);

    assert_eq!(
        repo.stash_paths(&["file0.txt".to_owned()], "gone").unwrap(),
        None
    );
    assert!(repo.stashes().unwrap().is_empty());
}
