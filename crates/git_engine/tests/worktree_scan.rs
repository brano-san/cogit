#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Remove Worktree scan, one stage at a time: changes, submodules, unpushed commits.

use git_engine::{FileStatus, RepoHandle};
use std::path::Path;

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn scan_of(f: &test_fixtures::Fixture, path: &str) -> git_engine::WorktreeScan {
    RepoHandle::open(f.path())
        .unwrap()
        .worktree_scan(path)
        .unwrap()
}

fn plain_worktree() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::with_worktree().unwrap();
    let path = RepoHandle::open(f.path())
        .unwrap()
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap()
        .path;
    (f, path)
}

/// `with_submodule` plus a worktree of it with the submodule checked out.
fn with_modules() -> (test_fixtures::Fixture, tempfile::TempDir, String) {
    let f = test_fixtures::with_submodule().unwrap();
    let aux = tempfile::TempDir::new().unwrap();
    let path = slashed(&aux.path().join("modules-wt"));
    f.git(&["worktree", "add", "-b", "modules-wt", &path])
        .unwrap();
    f.git_in(
        Path::new(&path),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        ],
    )
    .unwrap();
    (f, aux, path)
}

fn commit_in_submodule(path: &str) {
    let inside = Path::new(path).join("vendor/lib");
    std::fs::write(inside.join("local.txt"), "mine\n").unwrap();
    let git = |args: &[&str]| {
        let out = test_fixtures::git_command_in(&inside)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["add", "local.txt"]);
    git(&["commit", "-qm", "local work"]);
}

#[test]
fn changes_list_edits_and_untracked_files() {
    let (f, path) = plain_worktree();
    std::fs::write(Path::new(&path).join("file0.txt"), "work\n").unwrap();
    std::fs::write(Path::new(&path).join("new.txt"), "new\n").unwrap();

    let changes = scan_of(&f, &path).changes().unwrap();
    let found: Vec<(&str, FileStatus)> = changes
        .iter()
        .map(|c| (c.path.as_str(), c.status))
        .collect();

    assert!(
        found.contains(&("file0.txt", FileStatus::Modified)),
        "{found:?}"
    );
    assert!(
        found.contains(&("new.txt", FileStatus::Untracked)),
        "{found:?}"
    );
}

#[test]
fn a_clean_worktree_has_no_changes_submodules_or_unpushed_commits() {
    let (f, path) = plain_worktree();
    let scan = scan_of(&f, &path);

    assert!(scan.changes().unwrap().is_empty());
    assert!(scan.submodules().unwrap().paths.is_empty());
    assert!(scan.unpushed().unwrap().is_empty());
}

#[test]
fn the_changes_stage_leaves_submodules_to_their_own_stage() {
    let (f, _aux, path) = with_modules();
    std::fs::write(Path::new(&path).join("vendor/lib/loose.txt"), "x\n").unwrap();
    std::fs::write(Path::new(&path).join("README.md"), "edited\n").unwrap();
    let scan = scan_of(&f, &path);

    let paths: Vec<String> = scan
        .changes()
        .unwrap()
        .into_iter()
        .map(|c| c.path)
        .collect();
    assert_eq!(paths, ["README.md"]);

    let modules = scan.submodules().unwrap();
    assert_eq!(modules.paths, ["vendor/lib"]);
    assert_eq!(modules.changed.len(), 1, "{modules:?}");
    let row = &modules.changed[0];
    assert_eq!(row.path, "vendor/lib");
    assert!(row.submodule.as_ref().unwrap().untracked);
}

#[test]
fn a_clean_submodule_is_listed_but_not_changed() {
    let (f, _aux, path) = with_modules();
    let modules = scan_of(&f, &path).submodules().unwrap();

    assert_eq!(modules.paths, ["vendor/lib"]);
    assert!(modules.changed.is_empty(), "{modules:?}");
}

#[test]
fn a_submodule_with_a_new_commit_is_changed() {
    let (f, _aux, path) = with_modules();
    commit_in_submodule(&path);

    let modules = scan_of(&f, &path).submodules().unwrap();

    assert!(modules.changed[0].submodule.as_ref().unwrap().new_commits);
}

#[test]
fn unpushed_commits_of_a_submodule_are_counted_and_named() {
    let (f, _aux, path) = with_modules();
    // The submodule's `origin` is the fixture source: its own branch is on the remote.
    assert!(scan_of(&f, &path).unpushed().unwrap().is_empty());
    commit_in_submodule(&path);

    let unpushed = scan_of(&f, &path).unpushed().unwrap();

    assert_eq!(unpushed.len(), 1, "{unpushed:?}");
    assert_eq!(unpushed[0].path, "vendor/lib");
    assert_eq!(unpushed[0].total, 1);
    assert_eq!(unpushed[0].commits[0].summary, "local work");
}

#[test]
fn a_commit_on_another_local_branch_of_a_submodule_counts_too() {
    let (f, _aux, path) = with_modules();
    let inside = Path::new(&path).join("vendor/lib");
    let git = |args: &[&str]| {
        let out = test_fixtures::git_command_in(&inside)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["checkout", "-q", "-b", "side"]);
    std::fs::write(inside.join("side.txt"), "s\n").unwrap();
    git(&["add", "side.txt"]);
    git(&["commit", "-qm", "on side"]);
    git(&["checkout", "-q", "--detach", "origin/HEAD"]);

    let unpushed = scan_of(&f, &path).unpushed().unwrap();

    assert_eq!(unpushed[0].total, 1, "{unpushed:?}");
}

#[test]
fn a_path_that_is_not_a_worktree_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    let err = RepoHandle::open(f.path())
        .unwrap()
        .worktree_scan("Z:/nowhere");
    assert!(err.is_err());
}
