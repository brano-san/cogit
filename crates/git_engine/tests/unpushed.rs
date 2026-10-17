// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use test_fixtures::Fixture;

fn ours(f: &Fixture) -> Vec<String> {
    let mut out = RepoHandle::open(f.path()).unwrap().unpushed().unwrap();
    out.sort();
    out
}

fn rev_list(f: &Fixture, args: &[&str]) -> Vec<String> {
    let mut all = vec!["rev-list"];
    all.extend_from_slice(args);
    let mut out: Vec<String> = f.git(&all).unwrap().lines().map(str::to_owned).collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn local_commits_no_remote_branch_reaches_are_unpushed() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["branch", "side", "HEAD~1"]).unwrap();
    f.git(&["checkout", "-q", "-b", "fresh"]).unwrap();
    f.commit_file(40, "fresh.txt", "fresh\n").unwrap();

    let git = rev_list(&f, &["--branches", "--not", "--remotes"]);
    assert_eq!(git.len(), 3, "{git:?}");
    assert_eq!(ours(&f), git);
}

#[test]
fn a_detached_head_adds_its_own_commits() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["checkout", "-q", "--detach"]).unwrap();
    f.commit_file(41, "loose.txt", "on a detached head\n")
        .unwrap();

    let mut git = rev_list(&f, &["--branches", "--not", "--remotes"]);
    git.extend(rev_list(&f, &["HEAD", "--not", "--remotes"]));
    git.sort();
    git.dedup();
    assert_eq!(git.len(), 3, "{git:?}");
    assert_eq!(ours(&f), git);
}

#[test]
fn everything_pushed_leaves_nothing() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["push", "-q", "--force", "origin", "main"]).unwrap();
    f.git(&["fetch", "-q", "origin"]).unwrap();

    assert!(ours(&f).is_empty());
}

#[test]
fn without_a_remote_nothing_counts_as_unpushed() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(ours(&f).is_empty());
}
