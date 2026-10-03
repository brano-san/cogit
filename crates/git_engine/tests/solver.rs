// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the Conflict Solver reads from git: the names of "ours" and "theirs" for every
//! operation that can stop on a conflict, the sides of the awkward conflicts (modify/delete,
//! add/add, rename), the configured merge tool, and marking a file resolved.

use git_engine::{ConflictOperation, ConflictSide, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn two_sides() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(10, "file0.txt", "their line\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(11, "file0.txt", "our line\n").unwrap();
    f
}

#[test]
fn a_merge_names_our_branch_and_the_branch_merged_in() {
    let f = two_sides();
    let _ = f.git(&["merge", "theirs"]);

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::Merge);
    assert_eq!(context.ours, "main");
    assert_eq!(context.theirs, "theirs");
}

#[test]
fn a_merge_of_a_remote_tracking_branch_names_it_whole() {
    let f = two_sides();
    f.git(&["update-ref", "refs/remotes/origin/dev", "theirs"])
        .unwrap();
    let _ = f.git(&["merge", "origin/dev"]);

    assert_eq!(open(&f).conflict_context("file0.txt").theirs, "origin/dev");
}

#[test]
fn a_cherry_pick_names_the_commit_picked() {
    let f = two_sides();
    let _ = f.git(&["cherry-pick", "theirs"]);

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::CherryPick);
    assert_eq!(context.ours, "main");
    assert!(context.theirs.ends_with("commit 10"), "{}", context.theirs);
    assert_eq!(context.theirs.chars().take_while(|c| *c != ' ').count(), 7);
}

#[test]
fn a_revert_names_the_commit_reverted() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, "file0.txt", "second\n").unwrap();
    let reverted = f.commit_file(11, "file0.txt", "third\n").unwrap();
    let first = f.oid("HEAD~1").unwrap();
    assert_ne!(first, reverted);
    let _ = f.git(&["revert", "--no-edit", "HEAD~1"]);

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::Revert);
    assert!(
        context.theirs.starts_with("Revert of "),
        "{}",
        context.theirs
    );
}

#[test]
fn a_rebase_has_the_new_base_as_ours_and_the_commit_being_applied_as_theirs() {
    let f = two_sides();
    f.git(&["switch", "theirs"]).unwrap();
    let _ = f.git(&["rebase", "main"]);

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::Rebase);
    assert_eq!(context.ours, "main");
    assert!(context.theirs.ends_with("commit 10"), "{}", context.theirs);
}

#[test]
fn a_stash_apply_leaves_only_its_markers_to_name_the_side() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("file0.txt", "stashed\n").unwrap();
    f.git(&["stash", "push"]).unwrap();
    f.commit_file(12, "file0.txt", "committed\n").unwrap();
    let _ = f.git(&["stash", "apply"]);

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::StashApply);
    assert_eq!(context.ours, "main");
    assert_eq!(context.theirs, "Stashed changes");
}

#[test]
fn an_unnamed_conflict_still_has_sides_to_show() {
    let f = test_fixtures::linear(1).unwrap();

    let context = open(&f).conflict_context("file0.txt");

    assert_eq!(context.operation, ConflictOperation::Unknown);
    assert_eq!(context.ours, "main");
    assert_eq!(context.theirs, "theirs");
}

const NAVBOARD: &str = "src/device/src/plate/plate_navboard2.cpp";

/// Ours deleted a file that theirs changed: `plate_navboard2.cpp` of the field report.
fn modify_delete() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, NAVBOARD, "int a;\nint b;\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, NAVBOARD, "int a;\nint b = 2;\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["rm", "-q", "--", NAVBOARD]).unwrap();
    f.commit_staged(12, "remove the plate").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    f
}

#[test]
fn a_file_deleted_on_our_side_has_no_ours_but_keeps_its_base_and_theirs() {
    let f = modify_delete();
    let repo = open(&f);

    assert_eq!(repo.conflicted_paths().unwrap(), [NAVBOARD]);
    let sides = repo.conflict_sides(NAVBOARD).unwrap();

    assert!(sides.ours.is_none());
    assert_eq!(sides.base.as_deref(), Some(&b"int a;\nint b;\n"[..]));
    assert_eq!(sides.theirs.as_deref(), Some(&b"int a;\nint b = 2;\n"[..]));
}

#[test]
fn keeping_the_surviving_side_of_a_modify_delete_checks_it_out_and_stages_it() {
    let f = modify_delete();
    let repo = open(&f);

    repo.resolve_with(NAVBOARD, ConflictSide::Theirs, None)
        .unwrap();

    assert!(repo.conflicted_paths().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(f.path().join(NAVBOARD)).unwrap(),
        "int a;\nint b = 2;\n"
    );
    assert!(f.git(&["status", "--short"]).unwrap().contains("A  "));
}

#[test]
fn taking_the_missing_side_deletes_the_file() {
    let f = modify_delete();
    let repo = open(&f);

    repo.resolve_with(NAVBOARD, ConflictSide::Ours, None)
        .unwrap();

    assert!(repo.conflicted_paths().unwrap().is_empty());
    assert!(!f.path().join(NAVBOARD).exists());
}

#[test]
fn a_file_deleted_on_their_side_has_no_theirs() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, "gone.txt", "one\ntwo\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.git(&["rm", "-q", "--", "gone.txt"]).unwrap();
    f.commit_staged(11, "drop it").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "gone.txt", "one\nTWO\n").unwrap();
    let _ = f.git(&["merge", "theirs"]);

    let sides = open(&f).conflict_sides("gone.txt").unwrap();

    assert!(sides.theirs.is_none());
    assert_eq!(sides.ours.as_deref(), Some(&b"one\nTWO\n"[..]));
}

#[test]
fn add_add_has_two_sides_and_no_base() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(10, "new.txt", "their new file\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(11, "new.txt", "our new file\n").unwrap();
    let _ = f.git(&["merge", "theirs"]);

    let sides = open(&f).conflict_sides("new.txt").unwrap();

    assert!(sides.base.is_none());
    assert_eq!(sides.ours.as_deref(), Some(&b"our new file\n"[..]));
    assert_eq!(sides.theirs.as_deref(), Some(&b"their new file\n"[..]));
}

#[test]
fn a_rename_on_one_side_and_an_edit_on_the_other_conflict_under_the_new_name() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, "old.txt", "l1\nl2\nl3\nl4\nl5\nl6\n")
        .unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.git(&["mv", "old.txt", "renamed.txt"]).unwrap();
    f.write_file("renamed.txt", "l1\nl2\nl3\nl4\nl5\ntheirs\n")
        .unwrap();
    f.git(&["add", "--", "renamed.txt"]).unwrap();
    f.commit_staged(11, "rename and edit").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "old.txt", "l1\nl2\nl3\nl4\nl5\nours\n")
        .unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let repo = open(&f);

    let conflicted = repo.conflicted_paths().unwrap();
    assert!(!conflicted.is_empty(), "{conflicted:?}");
    for path in &conflicted {
        let sides = repo.conflict_sides(path).unwrap();
        assert!(
            sides.ours.is_some() || sides.theirs.is_some(),
            "{path} has nothing to show"
        );
    }
}

#[test]
fn a_file_with_its_markers_removed_is_marked_resolved() {
    let f = two_sides();
    let _ = f.git(&["merge", "theirs"]);
    let repo = open(&f);
    f.write_file("file0.txt", "settled by hand\n").unwrap();

    repo.mark_resolved("file0.txt").unwrap();

    assert!(repo.conflicted_paths().unwrap().is_empty());
    assert!(
        f.git(&["status", "--short"])
            .unwrap()
            .contains("M  file0.txt")
    );
}

#[test]
fn marking_a_file_that_is_not_conflicted_is_refused() {
    let f = two_sides();
    let _ = f.git(&["merge", "theirs"]);

    assert!(open(&f).mark_resolved("no-such.txt").is_err());
}

#[test]
fn the_merge_tool_set_in_git_is_read() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "merge.tool", "meld"]).unwrap();
    f.git(&[
        "config",
        "mergetool.meld.path",
        "C:/Program Files/Meld/Meld.exe",
    ])
    .unwrap();

    let config = open(&f).merge_tool_config().unwrap();

    assert_eq!(config.tool.as_deref(), Some("meld"));
    assert_eq!(
        config.path.as_deref(),
        Some("C:/Program Files/Meld/Meld.exe")
    );
}

#[test]
fn without_a_merge_tool_the_config_is_empty() {
    let f = test_fixtures::linear(1).unwrap();

    assert_eq!(open(&f).merge_tool_config().unwrap().tool, None);
}
