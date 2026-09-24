// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, StashOptions};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn dirty(f: &test_fixtures::Fixture, name: &str, body: &str) {
    std::fs::write(f.path().join(name), body).unwrap();
}

#[test]
fn a_repository_without_stashes_lists_none() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).stashes().unwrap().is_empty());
}

#[test]
fn stashes_are_listed_newest_first_with_their_message() {
    let f = test_fixtures::with_stashes(3).unwrap();

    let stashes = open(&f).stashes().unwrap();

    assert_eq!(stashes.len(), 3);
    assert_eq!(stashes[0].index, 0);
    assert!(!stashes[0].message.is_empty());
    assert_eq!(stashes[0].oid.len(), 40);
}

#[test]
fn stashing_everything_cleans_the_working_tree() {
    let f = test_fixtures::linear(2).unwrap();
    dirty(&f, "file0.txt", "work in progress\n");
    let repo = open(&f);

    repo.stash_push(&StashOptions {
        message: "wip".to_owned(),
        include_untracked: false,
        keep_index: false,
    })
    .unwrap();

    assert!(repo.worktree_files().unwrap().unstaged.is_empty());
    assert_eq!(repo.stashes().unwrap().len(), 1);
}

#[test]
fn the_message_is_kept() {
    let f = test_fixtures::linear(2).unwrap();
    dirty(&f, "file0.txt", "wip\n");
    let repo = open(&f);

    repo.stash_push(&StashOptions {
        message: "half of the parser".to_owned(),
        include_untracked: false,
        keep_index: false,
    })
    .unwrap();

    assert!(
        repo.stashes().unwrap()[0]
            .message
            .contains("half of the parser"),
        "{:?}",
        repo.stashes().unwrap()[0]
    );
}

#[test]
fn untracked_files_are_left_alone_unless_asked_for() {
    let f = test_fixtures::linear(2).unwrap();
    dirty(&f, "file0.txt", "tracked edit\n");
    dirty(&f, "fresh.txt", "untracked\n");
    let repo = open(&f);

    repo.stash_push(&StashOptions {
        message: "tracked only".to_owned(),
        include_untracked: false,
        keep_index: false,
    })
    .unwrap();

    assert!(f.path().join("fresh.txt").exists());
}

#[test]
fn untracked_files_are_taken_when_asked_for() {
    let f = test_fixtures::linear(2).unwrap();
    dirty(&f, "fresh.txt", "untracked\n");
    let repo = open(&f);

    repo.stash_push(&StashOptions {
        message: "with untracked".to_owned(),
        include_untracked: true,
        keep_index: false,
    })
    .unwrap();

    assert!(!f.path().join("fresh.txt").exists());
}

#[test]
fn keep_index_leaves_the_staged_part_in_place() {
    let f = test_fixtures::linear(2).unwrap();
    dirty(&f, "file0.txt", "staged\n");
    f.git(&["add", "--", "file0.txt"]).unwrap();
    let repo = open(&f);

    repo.stash_push(&StashOptions {
        message: "keep what is staged".to_owned(),
        include_untracked: false,
        keep_index: true,
    })
    .unwrap();

    assert_eq!(repo.worktree_files().unwrap().staged.len(), 1);
}

#[test]
fn stashing_a_clean_tree_is_refused_rather_than_silently_doing_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(
        repo.stash_push(&StashOptions {
            message: "nothing here".to_owned(),
            include_untracked: false,
            keep_index: false,
        })
        .is_err()
    );
}

#[test]
fn applying_a_stash_keeps_it_in_the_list() {
    let f = test_fixtures::with_stashes(2).unwrap();
    let repo = open(&f);

    repo.stash_apply_index(0, false).unwrap();

    assert_eq!(repo.stashes().unwrap().len(), 2);
}

#[test]
fn popping_a_stash_removes_it_from_the_list() {
    let f = test_fixtures::with_stashes(2).unwrap();
    let repo = open(&f);

    repo.stash_apply_index(0, true).unwrap();

    assert_eq!(repo.stashes().unwrap().len(), 1);
}

#[test]
fn dropping_a_stash_removes_it_without_touching_the_working_tree() {
    let f = test_fixtures::with_stashes(2).unwrap();
    let repo = open(&f);
    let before = repo.worktree_files().unwrap();

    repo.stash_drop(0).unwrap();

    assert_eq!(repo.stashes().unwrap().len(), 1);
    let after = repo.worktree_files().unwrap();
    assert_eq!(before.unstaged.len(), after.unstaged.len());
}

#[test]
fn dropping_reports_the_oid_so_undo_can_bring_it_back() {
    let f = test_fixtures::with_stashes(1).unwrap();
    let repo = open(&f);
    let oid = repo.stashes().unwrap()[0].oid.clone();

    let dropped = repo.stash_drop(0).unwrap();

    assert_eq!(dropped, oid);
}

#[test]
fn an_index_that_does_not_exist_is_a_typed_error() {
    let f = test_fixtures::with_stashes(1).unwrap();
    let repo = open(&f);

    assert!(repo.stash_drop(7).is_err());
    assert!(repo.stash_apply_index(7, false).is_err());
}

// Entries whose commit is gone are left out of the list, and the rest were numbered after
// that: from there on the list's numbers were not git's `stash@{n}`, and Drop removed a
// different stash from the one shown.
#[test]
fn a_stash_keeps_git_s_number_when_an_entry_above_it_is_unreadable() {
    let f = test_fixtures::linear(1).unwrap();
    for text in ["first", "second"] {
        f.write_file("file0.txt", &format!("{text}\n")).unwrap();
        f.git(&["stash", "push", "-q", "-m", text]).unwrap();
    }
    let log = f.git_dir().join("logs/refs/stash");
    let lines: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    // A chain git itself accepts: each entry starts where the one before it ended.
    let missing = "1234567890".repeat(4);
    let first_new = lines[0].split(' ').nth(1).unwrap().to_owned();
    let gone = format!(
        "{first_new} {missing} T <t@example.com> 1700000000 +0000	On main: a stash whose commit is gone"
    );
    let (_, second_rest) = lines[1].split_once(' ').unwrap();
    std::fs::write(
        &log,
        format!(
            "{}
{gone}
{missing} {second_rest}
",
            lines[0]
        ),
    )
    .unwrap();

    let listed = git_engine::RepoHandle::open(f.path())
        .unwrap()
        .stashes()
        .unwrap();

    let first = listed
        .iter()
        .find(|entry| entry.message.contains("first"))
        .unwrap();
    assert_eq!(first.index, 2, "{listed:?}");
    assert_eq!(
        first.oid,
        f.git(&["rev-parse", "stash@{2}"]).unwrap().trim()
    );
}
