// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! "Is there anything to pull" for a row of Repositories, against what console git says:
//! the server's tip of the upstream branch (`git ls-remote --heads`) is not in HEAD
//! (R-354). `None` — no upstream, or the server has no such branch.

use git_engine::{RepoHandle, pulse};
use test_fixtures::Fixture;

/// The answer from console git alone.
fn git_says(f: &Fixture) -> Option<bool> {
    let remote = f
        .git(&["config", "branch.main.remote"])
        .ok()?
        .trim()
        .to_owned();
    let merge = f
        .git(&["config", "branch.main.merge"])
        .ok()?
        .trim()
        .to_owned();
    let listed = f.git(&["ls-remote", "--heads", &remote]).unwrap();
    let tip = listed.lines().find_map(|line| {
        let (oid, name) = line.split_once('\t')?;
        (name == merge).then(|| oid.to_owned())
    })?;
    if f.git(&["cat-file", "-e", &format!("{tip}^{{commit}}")])
        .is_err()
    {
        return Some(true);
    }
    Some(
        f.git(&["merge-base", "--is-ancestor", &tip, "HEAD"])
            .is_err(),
    )
}

/// Before R-354 the row learnt it by fetching and counting against the tracking ref.
fn fetched_says(f: &Fixture) -> Option<bool> {
    RepoHandle::open(f.path())
        .unwrap()
        .background_fetch()
        .unwrap();
    let found = pulse(f.path());
    found.tracked.then_some(found.behind > 0)
}

/// Pushes one commit to `origin` from a clone of its own, so the local tracking ref is stale.
fn remote_moves_on(f: &Fixture, reset_to: Option<&str>) {
    let url = f
        .git(&["remote", "get-url", "origin"])
        .unwrap()
        .trim()
        .to_owned();
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other");
    f.git_in(
        dir.path(),
        &["clone", "--quiet", &url, &other.to_string_lossy()],
    )
    .unwrap();
    let identity = ["-c", "user.name=Other", "-c", "user.email=other@cogit.test"];
    match reset_to {
        None => {
            std::fs::write(other.join("later.txt"), "later\n").unwrap();
            f.git_in(&other, &["add", "--", "later.txt"]).unwrap();
            let mut args = identity.to_vec();
            args.extend(["commit", "--quiet", "-m", "later remote commit"]);
            f.git_in(&other, &args).unwrap();
            f.git_in(&other, &["push", "--quiet"]).unwrap();
        }
        Some(oid) => {
            f.git_in(&other, &["reset", "--quiet", "--hard", oid])
                .unwrap();
            f.git_in(&other, &["push", "--quiet", "--force"]).unwrap();
        }
    }
}

fn agree(f: &Fixture, expected: Option<bool>) {
    let truth = git_says(f);
    assert_eq!(truth, expected, "the fixture is not what the test thinks");
    assert_eq!(fetched_says(f), truth);
}

#[test]
fn behind_the_tracking_ref_there_is_something_to_pull() {
    let f = test_fixtures::with_remote().unwrap();
    agree(&f, Some(true));
}

#[test]
fn a_remote_that_moved_since_the_last_fetch_has_something_to_pull() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["merge", "--quiet", "--no-edit", "origin/main"])
        .unwrap();
    remote_moves_on(&f, None);
    agree(&f, Some(true));
}

#[test]
fn with_the_remote_merged_there_is_nothing_to_pull() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["merge", "--quiet", "--no-edit", "origin/main"])
        .unwrap();
    agree(&f, Some(false));
}

#[test]
fn a_remote_forced_back_to_a_commit_head_has_nothing_to_pull() {
    let f = test_fixtures::with_remote().unwrap();
    let first = f.oid("HEAD~2").unwrap();
    remote_moves_on(&f, Some(&first));
    agree(&f, Some(false));
}

#[test]
fn a_branch_without_an_upstream_has_no_answer() {
    let f = test_fixtures::linear(2).unwrap();
    assert_eq!(git_says(&f), None);
    assert!(!pulse(f.path()).tracked);
}
