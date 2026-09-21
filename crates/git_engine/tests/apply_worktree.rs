// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Discard throws lines away in the working tree. `apply_patch` stages, which leaves the
//! file on disk alone; this is the other target, and the one that loses work if it is wrong.

use git_engine::{PatchTarget, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// Two added lines on top of the committed content, so one can be thrown away.
fn two_added_lines() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "content 0\nfirst added\nsecond added\n",
    )
    .unwrap();
    f
}

const FIRST_ADDED: &str =
    "--- a/file0.txt\n+++ b/file0.txt\n@@ -1,1 +1,2 @@\n content 0\n+first added\n";

#[test]
fn discarding_a_line_removes_it_from_the_file_on_disk() {
    let f = two_added_lines();

    open(&f)
        .apply_patch_to(FIRST_ADDED, true, PatchTarget::WorkTree)
        .unwrap();

    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert_eq!(text, "content 0\nsecond added\n");
}

#[test]
fn discarding_one_line_leaves_the_others_alone() {
    let f = two_added_lines();

    open(&f)
        .apply_patch_to(FIRST_ADDED, true, PatchTarget::WorkTree)
        .unwrap();

    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert!(text.contains("second added"), "{text}");
}

#[test]
fn discarding_does_not_stage_anything() {
    let f = two_added_lines();
    let repo = open(&f);

    repo.apply_patch_to(FIRST_ADDED, true, PatchTarget::WorkTree)
        .unwrap();

    assert!(
        repo.worktree_files().unwrap().staged.is_empty(),
        "throwing work away is not the same as staging it"
    );
}

#[test]
fn the_index_target_still_leaves_the_file_on_disk_alone() {
    let f = two_added_lines();

    open(&f)
        .apply_patch_to(FIRST_ADDED, false, PatchTarget::Index)
        .unwrap();

    let text = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
    assert_eq!(text, "content 0\nfirst added\nsecond added\n");
}

#[test]
fn an_empty_patch_is_refused_before_git_sees_it() {
    let f = two_added_lines();

    let failed = open(&f).apply_patch_to("   \n", true, PatchTarget::WorkTree);

    assert!(failed.is_err());
}

#[test]
fn a_patch_that_does_not_fit_the_file_is_a_typed_error_with_gits_own_words() {
    let f = two_added_lines();
    let wrong = "--- a/file0.txt\n+++ b/file0.txt\n@@ -1,1 +1,2 @@\n nothing like it\n+nor this\n";

    let failed = open(&f).apply_patch_to(wrong, true, PatchTarget::WorkTree);

    match failed {
        Err(git_engine::GitError::Command(err)) => {
            assert!(!err.stderr.is_empty(), "git's own message must survive");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}
