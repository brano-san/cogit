#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Worktrees as things the user switches between, not as a command they remember (M3 T3.5).

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_plain_repository_has_exactly_one_worktree() {
    let f = test_fixtures::linear(1).unwrap();
    let found = open(&f).worktrees().unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].is_main);
}

#[test]
fn a_linked_worktree_is_listed_beside_the_main_one() {
    let f = test_fixtures::with_worktree().unwrap();
    let found = open(&f).worktrees().unwrap();
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found.iter().filter(|entry| entry.is_main).count(), 1);
}

#[test]
fn each_worktree_names_the_branch_it_has_checked_out() {
    let f = test_fixtures::with_worktree().unwrap();
    let found = open(&f).worktrees().unwrap();
    let branches: Vec<Option<String>> = found.iter().map(|entry| entry.branch.clone()).collect();
    assert!(branches.contains(&Some("main".to_owned())), "{branches:?}");
    assert!(
        branches.contains(&Some("feature-wt".to_owned())),
        "{branches:?}"
    );
}

#[test]
fn the_handles_own_worktree_is_marked_current() {
    let f = test_fixtures::with_worktree().unwrap();
    let found = open(&f).worktrees().unwrap();
    let current: Vec<&str> = found
        .iter()
        .filter(|entry| entry.is_current)
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(current.len(), 1, "{found:?}");
}

#[test]
fn a_worktree_whose_folder_is_gone_is_reported_missing() {
    let f = test_fixtures::with_worktree().unwrap();
    let linked = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap();
    std::fs::remove_dir_all(&linked.path).unwrap();

    let after = open(&f).worktrees().unwrap();
    let gone = after
        .iter()
        .find(|entry| entry.path == linked.path)
        .unwrap();
    assert!(gone.missing, "{after:?}");
}

#[test]
fn a_locked_worktree_reports_its_reason() {
    let f = test_fixtures::with_worktree().unwrap();
    let linked = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap();
    f.git(&[
        "worktree",
        "lock",
        "--reason",
        "on a usb stick",
        &linked.path,
    ])
    .unwrap();

    let after = open(&f).worktrees().unwrap();
    let locked = after
        .iter()
        .find(|entry| entry.path == linked.path)
        .unwrap();
    assert_eq!(locked.locked.as_deref(), Some("on a usb stick"));
}

#[test]
fn an_unlocked_worktree_says_nothing_about_locks() {
    let f = test_fixtures::with_worktree().unwrap();
    assert!(
        open(&f)
            .worktrees()
            .unwrap()
            .iter()
            .all(|entry| entry.locked.is_none())
    );
}

#[test]
fn a_dirty_worktree_is_told_apart_from_a_clean_one() {
    let f = test_fixtures::with_worktree().unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();

    let found = open(&f).worktrees().unwrap();
    let main = found.iter().find(|entry| entry.is_main).unwrap();
    let linked = found.iter().find(|entry| !entry.is_main).unwrap();
    assert!(main.dirty, "{found:?}");
    assert!(!linked.dirty, "{found:?}");
}

#[test]
fn adding_a_worktree_puts_a_branch_in_it() {
    let f = test_fixtures::linear(2).unwrap();
    let target = f.path().parent().unwrap().join("added-wt");
    let path = target.to_string_lossy().replace('\\', "/");

    open(&f).add_worktree(&path, "spare", true).unwrap();

    let found = open(&f).worktrees().unwrap();
    assert!(
        found
            .iter()
            .any(|entry| entry.branch.as_deref() == Some("spare")),
        "{found:?}"
    );
    std::fs::remove_dir_all(&target).ok();
}

#[test]
fn removing_a_worktree_takes_it_off_the_list() {
    let f = test_fixtures::with_worktree().unwrap();
    let linked = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap();

    open(&f).remove_worktree(&linked.path, false).unwrap();
    assert_eq!(open(&f).worktrees().unwrap().len(), 1);
}

#[test]
fn removing_a_worktree_with_changes_is_refused_until_forced() {
    let f = test_fixtures::with_worktree().unwrap();
    let linked = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap();
    std::fs::write(
        std::path::Path::new(&linked.path).join("file0.txt"),
        "work\n",
    )
    .unwrap();

    assert!(open(&f).remove_worktree(&linked.path, false).is_err());
    open(&f).remove_worktree(&linked.path, true).unwrap();
    assert_eq!(open(&f).worktrees().unwrap().len(), 1);
}

#[test]
fn pruning_forgets_a_worktree_whose_folder_vanished() {
    let f = test_fixtures::with_worktree().unwrap();
    let linked = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap();
    std::fs::remove_dir_all(&linked.path).unwrap();

    open(&f).prune_worktrees().unwrap();
    assert_eq!(open(&f).worktrees().unwrap().len(), 1);
}

#[test]
fn the_branch_a_worktree_holds_can_be_found_by_name() {
    let f = test_fixtures::with_worktree().unwrap();
    let repo = open(&f);
    assert!(repo.worktree_holding("feature-wt").unwrap().is_some());
    assert!(repo.worktree_holding("no-such-branch").unwrap().is_none());
}

#[test]
fn the_branch_of_the_current_worktree_is_not_reported_as_held_elsewhere() {
    let f = test_fixtures::with_worktree().unwrap();
    assert_eq!(open(&f).worktree_holding("main").unwrap(), None);
}
