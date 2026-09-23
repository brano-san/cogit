// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Remote ▸ Subtree (#45), over `git subtree`.

use git_engine::{GitError, RepoHandle, SubtreeOp};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn url_of(f: &test_fixtures::Fixture) -> String {
    f.path().to_string_lossy().replace('\\', "/")
}

fn add(prefix: &str, source: &test_fixtures::Fixture, squash: bool) -> SubtreeOp {
    SubtreeOp::Add {
        prefix: prefix.to_owned(),
        repository: url_of(source),
        reference: "main".to_owned(),
        squash,
    }
}

/// A parent with `lib/` brought in from a two-commit repository.
fn with_subtree() -> (test_fixtures::Fixture, test_fixtures::Fixture) {
    let parent = test_fixtures::linear(1).unwrap();
    let source = test_fixtures::linear(2).unwrap();
    open(&parent)
        .subtree_op(&add("lib", &source, false))
        .unwrap();
    (parent, source)
}

#[test]
fn add_brings_another_repository_in_under_the_prefix() {
    let (parent, _source) = with_subtree();

    assert_eq!(
        std::fs::read_to_string(parent.path().join("lib/file1.txt")).unwrap(),
        "content 1\n"
    );
    let message = parent.git(&["log", "-1", "--format=%B"]).unwrap();
    assert!(message.contains("git-subtree-dir: lib"), "{message}");
}

#[test]
fn add_with_squash_brings_one_commit_instead_of_the_history() {
    let parent = test_fixtures::linear(1).unwrap();
    let source = test_fixtures::linear(2).unwrap();

    open(&parent)
        .subtree_op(&add("lib", &source, true))
        .unwrap();

    assert!(parent.path().join("lib/file0.txt").is_file());
    let count = parent.git(&["rev-list", "--count", "HEAD"]).unwrap();
    // Our commit, the squashed one and the merge: the source's two commits are not in.
    assert_eq!(count.trim(), "3");
}

#[test]
fn a_prefix_that_escapes_the_repository_is_refused() {
    let parent = test_fixtures::linear(1).unwrap();
    let source = test_fixtures::linear(2).unwrap();
    for prefix in ["", "  ", "../outside", "/abs", "a/../../b"] {
        let refused = open(&parent).subtree_op(&add(prefix, &source, false));
        assert!(
            matches!(refused, Err(GitError::InvalidState(_))),
            "{prefix}: {refused:?}"
        );
    }
}

#[test]
fn the_prefixes_added_are_found_again_in_the_history() {
    let (parent, _source) = with_subtree();

    assert_eq!(open(&parent).subtree_prefixes().unwrap(), ["lib"]);
}

#[test]
fn a_repository_without_subtrees_has_no_prefixes() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(open(&f).subtree_prefixes().unwrap().is_empty());
}

#[test]
fn merge_from_a_repository_brings_its_new_commits() {
    let (parent, source) = with_subtree();
    source.commit_file(5, "later.txt", "later\n").unwrap();

    open(&parent)
        .subtree_op(&SubtreeOp::Merge {
            prefix: "lib".to_owned(),
            repository: Some(url_of(&source)),
            reference: "main".to_owned(),
            squash: false,
        })
        .unwrap();

    assert!(parent.path().join("lib/later.txt").is_file());
}

#[test]
fn merge_of_a_local_commit_needs_no_repository() {
    let (parent, source) = with_subtree();
    source.commit_file(5, "later.txt", "later\n").unwrap();
    parent
        .git(&["fetch", &url_of(&source), "main:refs/remotes/lib/main"])
        .unwrap();

    open(&parent)
        .subtree_op(&SubtreeOp::Merge {
            prefix: "lib".to_owned(),
            repository: None,
            reference: "lib/main".to_owned(),
            squash: false,
        })
        .unwrap();

    assert!(parent.path().join("lib/later.txt").is_file());
}

#[test]
fn split_makes_a_branch_of_the_subtree_alone() {
    let (parent, _source) = with_subtree();

    open(&parent)
        .subtree_op(&SubtreeOp::Split {
            prefix: "lib".to_owned(),
            branch: "lib-only".to_owned(),
            rejoin: false,
        })
        .unwrap();

    let files = parent.git(&["ls-tree", "--name-only", "lib-only"]).unwrap();
    assert_eq!(
        files.lines().collect::<Vec<_>>(),
        ["file0.txt", "file1.txt"]
    );
}

#[test]
fn push_sends_the_subtree_to_a_repository() {
    let (parent, _source) = with_subtree();
    let target = test_fixtures::bare().unwrap();

    open(&parent)
        .subtree_op(&SubtreeOp::Push {
            prefix: "lib".to_owned(),
            repository: url_of(&target),
            reference: "lib".to_owned(),
        })
        .unwrap();

    let files = target.git(&["ls-tree", "--name-only", "lib"]).unwrap();
    assert!(files.contains("file1.txt"), "{files}");
}

#[test]
fn reset_puts_the_folder_back_as_the_commit_has_it_and_stages_it() {
    let (parent, source) = with_subtree();
    let original = source.oid("main").unwrap();
    parent
        .commit_file(9, "lib/file1.txt", "changed here\n")
        .unwrap();
    parent
        .commit_file(10, "lib/extra.txt", "added here\n")
        .unwrap();

    open(&parent)
        .subtree_op(&SubtreeOp::Reset {
            prefix: "lib".to_owned(),
            reference: original,
        })
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(parent.path().join("lib/file1.txt")).unwrap(),
        "content 1\n"
    );
    assert!(!parent.path().join("lib/extra.txt").exists());
    let staged = parent.git(&["diff", "--cached", "--name-status"]).unwrap();
    assert!(
        staged.contains("M\tlib/file1.txt") && staged.contains("D\tlib/extra.txt"),
        "{staged}"
    );
}

#[test]
fn reset_refuses_a_folder_with_uncommitted_work() {
    let (parent, source) = with_subtree();
    let original = source.oid("main").unwrap();
    parent
        .write_file("lib/file1.txt", "not committed\n")
        .unwrap();

    let refused = open(&parent).subtree_op(&SubtreeOp::Reset {
        prefix: "lib".to_owned(),
        reference: original,
    });

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "{refused:?}"
    );
    assert_eq!(
        std::fs::read_to_string(parent.path().join("lib/file1.txt")).unwrap(),
        "not committed\n"
    );
}
