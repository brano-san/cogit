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

fn linked(f: &test_fixtures::Fixture) -> git_engine::WorktreeEntry {
    open(f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap()
}

fn slashed(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Opened on a linked worktree the list used to call that one the main worktree, lose the
/// real main one and show the linked one twice (R-184).
#[test]
fn listed_from_a_linked_worktree_the_main_one_is_still_the_main_one() {
    let f = test_fixtures::with_worktree().unwrap();
    let path = linked(&f).path;

    let found = RepoHandle::open(std::path::Path::new(&path))
        .unwrap()
        .worktrees()
        .unwrap();

    assert_eq!(found.len(), 2, "{found:?}");
    let main = found.iter().find(|entry| entry.is_main).unwrap();
    assert_eq!(main.path, slashed(f.path()));
    assert!(!main.is_current);
    let here = found.iter().find(|entry| entry.path == path).unwrap();
    assert!(here.is_current && !here.is_main, "{here:?}");
}

/// The panel names a worktree by its folder; the full path is the tooltip (R-184).
#[test]
fn every_worktree_is_named_after_its_folder() {
    let f = test_fixtures::with_worktree().unwrap();
    let found = open(&f).worktrees().unwrap();
    let main = found.iter().find(|entry| entry.is_main).unwrap();
    let expected = f.path().file_name().unwrap().to_string_lossy();

    assert_eq!(main.name, expected);
    assert_eq!(linked(&f).name, "linked");
}

#[test]
fn a_worktree_can_be_locked_with_a_reason_and_unlocked_again() {
    let f = test_fixtures::with_worktree().unwrap();
    let path = linked(&f).path;

    open(&f)
        .lock_worktree(&path, Some("on a usb stick"))
        .unwrap();
    assert_eq!(linked(&f).locked.as_deref(), Some("on a usb stick"));

    open(&f).unlock_worktree(&path).unwrap();
    assert_eq!(linked(&f).locked, None);
}

#[test]
fn pruning_one_missing_worktree_leaves_the_other_registrations_alone() {
    let f = test_fixtures::with_worktree().unwrap();
    let aux = tempfile::tempdir().unwrap();
    let second = slashed(&aux.path().join("second"));
    open(&f).add_worktree(&second, "second", true).unwrap();
    let first = linked(&f).path;
    std::fs::remove_dir_all(&first).unwrap();
    std::fs::remove_dir_all(&second).unwrap();

    open(&f).prune_worktree(&first).unwrap();

    let left: Vec<String> = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .filter(|entry| !entry.is_main)
        .map(|entry| entry.path)
        .collect();
    assert_eq!(left, [second]);
}

#[test]
fn pruning_a_worktree_whose_folder_is_still_there_is_refused() {
    let f = test_fixtures::with_worktree().unwrap();
    let path = linked(&f).path;

    let refused = open(&f).prune_worktree(&path);

    assert!(
        matches!(refused, Err(git_engine::GitError::InvalidState(_))),
        "{refused:?}"
    );
    assert!(std::path::Path::new(&path).join("file0.txt").exists());
    assert_eq!(open(&f).worktrees().unwrap().len(), 2);
}

#[test]
fn a_moved_worktree_is_found_again_by_repairing_it_with_its_new_folder() {
    let f = test_fixtures::with_worktree().unwrap();
    let old = linked(&f).path;
    let aux = tempfile::tempdir().unwrap();
    let moved = aux.path().join("moved");
    std::fs::rename(&old, &moved).unwrap();
    assert!(linked(&f).missing);

    open(&f).repair_worktree(&slashed(&moved)).unwrap();

    let repaired = linked(&f);
    assert!(!repaired.missing, "{repaired:?}");
    assert_eq!(repaired.path, slashed(&moved));
    assert_eq!(repaired.branch.as_deref(), Some("feature-wt"));
}

/// `dtv_device_master`, registered on Linux as `/home/user/work/…`: Windows reads that as
/// `E:\home\user\work\…`, which is nowhere. Repair with the real folder closes it.
#[test]
fn a_worktree_registered_from_another_system_is_repaired_with_its_real_folder() {
    let f = test_fixtures::with_worktree().unwrap();
    let real = linked(&f).path;
    let admin = f.git_dir().join("worktrees").join("linked");
    std::fs::write(admin.join("gitdir"), "/home/user/work/linked/.git\n").unwrap();
    assert!(linked(&f).missing, "{:?}", linked(&f));

    open(&f).repair_worktree(&real).unwrap();

    let repaired = linked(&f);
    assert!(!repaired.missing, "{repaired:?}");
    assert_eq!(repaired.path, real);
}

#[test]
fn a_new_worktree_can_start_its_branch_at_a_chosen_commit() {
    let f = test_fixtures::linear(3).unwrap();
    let first = f.oid("HEAD~2").unwrap();
    let aux = tempfile::tempdir().unwrap();
    let path = slashed(&aux.path().join("from-first"));

    open(&f)
        .add_worktree_at(&path, "from-first", true, Some(&first))
        .unwrap();

    let added = open(&f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| entry.branch.as_deref() == Some("from-first"))
        .unwrap();
    assert_eq!(added.head, first);
}

#[test]
fn the_changes_in_a_worktree_are_listed_before_it_is_removed() {
    let f = test_fixtures::with_worktree().unwrap();
    let path = linked(&f).path;
    std::fs::write(std::path::Path::new(&path).join("file0.txt"), "work\n").unwrap();
    std::fs::write(std::path::Path::new(&path).join("new.txt"), "new\n").unwrap();

    let changes = open(&f).worktree_changes(&path).unwrap();
    let paths: Vec<&str> = changes.iter().map(|file| file.path.as_str()).collect();

    assert!(paths.contains(&"file0.txt"), "{paths:?}");
    assert!(paths.contains(&"new.txt"), "{paths:?}");
}

#[test]
fn the_changes_of_a_worktree_can_be_put_in_a_stash_before_it_goes() {
    let f = test_fixtures::with_worktree().unwrap();
    let path = linked(&f).path;
    std::fs::write(std::path::Path::new(&path).join("file0.txt"), "work\n").unwrap();

    let stashed = open(&f)
        .stash_worktree_changes(&path, "cogit: before removing worktree linked")
        .unwrap();

    assert!(stashed.is_some());
    assert!(open(&f).worktree_changes(&path).unwrap().is_empty());
    let messages: Vec<String> = open(&f)
        .stashes()
        .unwrap()
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("before removing worktree linked")),
        "{messages:?}"
    );
}

/// Branches marks a branch whose worktree folder is gone as `missing`, so the branch has to
/// survive the folder: git still refuses to check it out anywhere else.
#[test]
fn a_missing_worktree_still_names_the_branch_it_holds() {
    let f = test_fixtures::with_worktree().unwrap();
    std::fs::remove_dir_all(linked(&f).path).unwrap();

    let gone = linked(&f);
    assert!(gone.missing, "{gone:?}");
    assert_eq!(gone.branch.as_deref(), Some("feature-wt"), "{gone:?}");
    assert!(!gone.head.is_empty(), "{gone:?}");
}

// A bare repository's `.git` is not called `.git`: the main root fell back to the worktree
// asking, which then showed up twice, once as the main one, and the bare one not at all.
#[test]
fn a_worktree_of_a_bare_repository_lists_the_bare_one_as_main() {
    let bare = test_fixtures::bare().unwrap();
    let place = tempfile::tempdir().unwrap();
    let linked = place.path().join("wt");
    bare.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "wtb",
        &linked.to_string_lossy(),
    ])
    .unwrap();

    let entries = RepoHandle::open(&linked).unwrap().worktrees().unwrap();

    assert_eq!(entries.len(), 2, "{entries:?}");
    assert_eq!(
        entries.iter().filter(|entry| entry.is_current).count(),
        1,
        "{entries:?}"
    );
    let main = entries.iter().find(|entry| entry.is_main).unwrap();
    assert!(!main.is_current, "{entries:?}");
}
