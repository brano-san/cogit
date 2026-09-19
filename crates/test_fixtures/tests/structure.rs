// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Submodules and worktrees — the shapes that need a second repository on disk.

use test_fixtures::{with_submodule, with_worktree};

#[test]
fn submodule_is_recorded_in_gitmodules() {
    let f = with_submodule().unwrap();
    let modules = f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "--get",
        "submodule.vendor/lib.path",
    ]);
    assert_eq!(modules.unwrap().trim(), "vendor/lib");
}

#[test]
fn submodule_is_a_gitlink_not_a_directory() {
    // Mode 160000 is what makes it a submodule rather than a committed folder.
    let f = with_submodule().unwrap();
    let entry = f.git(&["ls-tree", "HEAD", "--", "vendor/lib"]).unwrap();
    assert!(
        entry.starts_with("160000"),
        "expected a gitlink, got: {entry}"
    );
}

#[test]
fn submodule_is_checked_out_with_its_own_history() {
    let f = with_submodule().unwrap();
    let status = f.git(&["submodule", "status"]).unwrap();
    // A leading '-' means "not initialised"; the fixture must hand over a usable one.
    assert!(
        !status.trim_start().starts_with('-'),
        "submodule not initialised: {status}"
    );
}

#[test]
fn worktree_is_listed_alongside_the_main_checkout() {
    let f = with_worktree().unwrap();
    let list = f.git(&["worktree", "list", "--porcelain"]).unwrap();
    let count = list.lines().filter(|l| l.starts_with("worktree ")).count();
    assert_eq!(
        count, 2,
        "expected the main checkout plus one worktree:\n{list}"
    );
}

#[test]
fn worktree_is_on_its_own_branch() {
    // Two checkouts of the same branch are impossible in Git, so the fixture has to
    // put the worktree on a branch of its own — exactly what M3 has to display.
    let f = with_worktree().unwrap();
    let list = f.git(&["worktree", "list", "--porcelain"]).unwrap();
    assert!(
        list.contains("refs/heads/feature-wt"),
        "missing worktree branch:\n{list}"
    );
}
