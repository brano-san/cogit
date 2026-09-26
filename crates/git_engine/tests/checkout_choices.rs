#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Checkout dialog's choices beyond a plain switch (item 40 of 25.09): a new local
//! branch, tracking the remote branch it starts from or not, and a local branch moved
//! forward to its remote branch before it is checked out.

use git_engine::{CheckoutTarget, GitError, Head, RepoHandle};

/// `main` at c2; `feature` at c0 tracks `origin/feature` at c1, so it is one behind.
/// The remote is never reached: its branch is written straight into `refs/remotes`.
fn behind_its_upstream() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ])
    .unwrap();
    let c1 = f.oid("main~1").unwrap();
    f.git(&["update-ref", "refs/remotes/origin/feature", &c1])
        .unwrap();
    f.git(&["branch", "--no-track", "feature", "main~2"])
        .unwrap();
    f.git(&["config", "branch.feature.remote", "origin"])
        .unwrap();
    f.git(&["config", "branch.feature.merge", "refs/heads/feature"])
        .unwrap();
    f
}

fn head(repo: &RepoHandle) -> (String, String) {
    match repo.head().unwrap() {
        Head::Branch { name, oid } => (name, oid),
        other => panic!("expected an attached HEAD, got {other:?}"),
    }
}

/// A handle of its own: one opened before the write keeps the config it read.
fn upstream_of(f: &test_fixtures::Fixture, branch: &str) -> Option<String> {
    RepoHandle::open(f.path())
        .unwrap()
        .branches()
        .unwrap()
        .into_iter()
        .find(|found| found.name == branch)
        .and_then(|found| found.upstream)
}

fn new_branch(name: &str, start: &str, track: bool) -> CheckoutTarget {
    CheckoutTarget::NewBranch {
        name: name.to_owned(),
        start: start.to_owned(),
        track,
    }
}

fn fast_forward(name: &str, to: &str) -> CheckoutTarget {
    CheckoutTarget::FastForward {
        name: name.to_owned(),
        to: to.to_owned(),
    }
}

#[test]
fn a_new_branch_tracks_the_remote_branch_it_starts_from() {
    let f = behind_its_upstream();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.checkout(&new_branch("review", "refs/remotes/origin/feature", true))
        .unwrap();

    assert_eq!(head(&repo), ("review".to_owned(), f.oid("main~1").unwrap()));
    assert_eq!(upstream_of(&f, "review").as_deref(), Some("origin/feature"));
}

// `branch.autoSetupMerge` defaults to tracking a branch started from a remote one: an
// unticked Track remote branch has to say so.
#[test]
fn a_new_branch_left_untracked_has_no_upstream() {
    let f = behind_its_upstream();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.checkout(&new_branch("review", "refs/remotes/origin/feature", false))
        .unwrap();

    assert_eq!(head(&repo).0, "review");
    assert_eq!(upstream_of(&f, "review"), None);
}

#[test]
fn a_new_branch_at_a_commit_starts_there() {
    let f = behind_its_upstream();
    let repo = RepoHandle::open(f.path()).unwrap();
    let c0 = f.oid("main~2").unwrap();

    repo.checkout(&new_branch("topic", &c0, false)).unwrap();

    assert_eq!(head(&repo), ("topic".to_owned(), c0));
    assert!(!f.path().join("file1.txt").exists());
}

#[test]
fn a_taken_name_is_refused_and_head_stays() {
    let f = behind_its_upstream();
    let repo = RepoHandle::open(f.path()).unwrap();

    let refused = repo.checkout(&new_branch("feature", "refs/remotes/origin/feature", true));

    assert!(matches!(refused, Err(GitError::Command(_))), "{refused:?}");
    assert_eq!(head(&repo).0, "main");
}

#[test]
fn fast_forward_moves_the_branch_then_checks_it_out() {
    let f = behind_its_upstream();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.checkout(&fast_forward("feature", "refs/remotes/origin/feature"))
        .unwrap();

    assert_eq!(
        head(&repo),
        ("feature".to_owned(), f.oid("main~1").unwrap())
    );
    assert!(f.path().join("file1.txt").exists());
    assert!(!f.path().join("file2.txt").exists());
    assert_eq!(
        upstream_of(&f, "feature").as_deref(),
        Some("origin/feature"),
        "moving the branch keeps what it tracks"
    );
}

#[test]
fn fast_forward_refuses_a_branch_that_has_diverged() {
    let f = behind_its_upstream();
    f.git(&["switch", "feature"]).unwrap();
    f.commit_file(5, "only-local.txt", "mine\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let before = f.oid("feature").unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let refused = repo.checkout(&fast_forward("feature", "refs/remotes/origin/feature"));

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "{refused:?}"
    );
    assert_eq!(head(&repo).0, "main");
    assert_eq!(f.oid("feature").unwrap(), before);
}

// `switch -C` moves the branch only once the working tree has switched, so a refusal
// leaves nothing half done.
#[test]
fn a_refused_fast_forward_leaves_the_branch_where_it_was() {
    let f = behind_its_upstream();
    // c1 has no file2.txt: the switch would have to delete the edited file.
    f.write_file("file2.txt", "in the way\n").unwrap();
    let before = f.oid("feature").unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let refused = repo.checkout(&fast_forward("feature", "refs/remotes/origin/feature"));

    assert!(refused.is_err());
    assert_eq!(head(&repo).0, "main");
    assert_eq!(f.oid("feature").unwrap(), before);
}

#[test]
fn the_checked_out_branch_fast_forwards_in_place() {
    let f = behind_its_upstream();
    f.git(&["switch", "feature"]).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.checkout(&fast_forward("feature", "refs/remotes/origin/feature"))
        .unwrap();

    assert_eq!(
        head(&repo),
        ("feature".to_owned(), f.oid("main~1").unwrap())
    );
    assert!(f.path().join("file1.txt").exists());
}
