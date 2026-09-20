#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A stash can be read without touching the working tree: it is an ordinary commit whose
//! parents hold the three parts git put away (M5 T5.2).

use git_engine::{FileStatus, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn paths(entries: &[git_engine::FileEntry]) -> Vec<String> {
    let mut found: Vec<String> = entries.iter().map(|entry| entry.path.clone()).collect();
    found.sort();
    found
}

/// Tracked edit, a staged edit and an untracked file, so all three parts are non-empty.
fn stashed() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "unstaged edit\n").unwrap();
    std::fs::write(f.path().join("file1.txt"), "staged edit\n").unwrap();
    f.git(&["add", "--", "file1.txt"]).unwrap();
    std::fs::write(f.path().join("brand-new.txt"), "never added\n").unwrap();
    f.git_at(10, &["stash", "push", "-u", "-m", "wip"]).unwrap();
    f
}

#[test]
fn the_working_tree_part_lists_what_was_only_edited() {
    let f = stashed();
    let parts = open(&f).stash_contents(0).unwrap();
    assert!(
        paths(&parts.worktree).contains(&"file0.txt".to_owned()),
        "{parts:?}"
    );
}

#[test]
fn the_index_part_lists_what_had_been_staged() {
    let f = stashed();
    let parts = open(&f).stash_contents(0).unwrap();
    assert_eq!(paths(&parts.index), ["file1.txt"]);
}

#[test]
fn the_untracked_part_lists_files_git_was_not_tracking() {
    let f = stashed();
    let parts = open(&f).stash_contents(0).unwrap();
    assert_eq!(paths(&parts.untracked), ["brand-new.txt"]);
}

#[test]
fn an_untracked_file_is_an_addition_not_a_modification() {
    let f = stashed();
    let parts = open(&f).stash_contents(0).unwrap();
    assert_eq!(parts.untracked[0].status, FileStatus::Added);
}

#[test]
fn reading_a_stash_leaves_the_working_tree_alone() {
    let f = stashed();
    let before = f.git(&["status", "--porcelain"]).unwrap();
    open(&f).stash_contents(0).unwrap();
    assert_eq!(f.git(&["status", "--porcelain"]).unwrap(), before);
}

#[test]
fn reading_a_stash_spawns_no_process() {
    let f = stashed();
    let repo = open(&f);
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = std::sync::Arc::clone(&log);
        std::sync::Arc::new(move |entry: git_engine::GitOutput| {
            log.lock().unwrap().push(entry);
        })
    };

    repo.with_journal(sink).stash_contents(0).unwrap();
    assert!(
        log.lock().unwrap().is_empty(),
        "reads go through gix (INV-03)"
    );
}

#[test]
fn the_revisions_let_the_caller_diff_each_part() {
    let f = stashed();
    let parts = open(&f).stash_contents(0).unwrap();

    // Every revision is a real commit the diff engine can resolve.
    for rev in [&parts.base, &parts.worktree_rev, &parts.index_rev] {
        assert_eq!(rev.len(), 40, "expected a full oid, got {rev}");
    }
    assert_eq!(parts.untracked_rev.as_ref().map(String::len), Some(40));
}

#[test]
fn a_stash_without_untracked_files_has_no_untracked_part() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    f.git_at(10, &["stash", "push", "-m", "plain"]).unwrap();

    let parts = open(&f).stash_contents(0).unwrap();
    assert!(parts.untracked.is_empty());
    assert_eq!(parts.untracked_rev, None);
}

#[test]
fn the_second_stash_is_reachable_by_its_index() {
    let f = test_fixtures::with_stashes(2).unwrap();
    let parts = open(&f).stash_contents(1).unwrap();
    assert!(
        !parts.worktree.is_empty() || !parts.untracked.is_empty(),
        "{parts:?}"
    );
}

#[test]
fn an_index_past_the_end_is_refused() {
    let f = test_fixtures::with_stashes(1).unwrap();
    assert!(open(&f).stash_contents(9).is_err());
}

#[test]
fn a_repository_without_stashes_refuses_rather_than_returning_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).stash_contents(0).is_err());
}
