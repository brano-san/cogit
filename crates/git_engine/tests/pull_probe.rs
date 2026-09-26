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

/// Every ref and `FETCH_HEAD`: the probe must leave all of them as they were.
fn refs_of(f: &Fixture) -> (String, bool) {
    (
        f.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
            .unwrap(),
        f.git_dir().join("FETCH_HEAD").exists(),
    )
}

fn agree(f: &Fixture, expected: Option<bool>) {
    let truth = git_says(f);
    assert_eq!(truth, expected, "the fixture is not what the test thinks");
    let before = refs_of(f);
    assert_eq!(
        RepoHandle::open(f.path()).unwrap().pull_probe().unwrap(),
        truth
    );
    assert_eq!(refs_of(f), before, "the probe wrote something");
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
    assert_eq!(
        RepoHandle::open(f.path()).unwrap().pull_probe().unwrap(),
        None
    );
    assert!(!pulse(f.path()).tracked);
}

// Only the probe: a fetch without prune keeps the stale tracking ref and says "behind".
#[test]
fn an_upstream_deleted_on_the_server_has_no_answer() {
    let f = test_fixtures::with_remote().unwrap();
    let url = f
        .git(&["remote", "get-url", "origin"])
        .unwrap()
        .trim()
        .to_owned();
    f.git_in(
        std::path::Path::new(&url),
        &["update-ref", "-d", "refs/heads/main"],
    )
    .unwrap();
    assert_eq!(git_says(&f), None);
    assert_eq!(
        RepoHandle::open(f.path()).unwrap().pull_probe().unwrap(),
        None
    );
}

#[test]
fn the_heads_are_what_git_ls_remote_lists() {
    let f = test_fixtures::with_remote().unwrap();
    remote_moves_on(&f, None);
    let git: Vec<String> = f
        .git(&["ls-remote", "--heads", "origin"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let ours: Vec<String> = RepoHandle::open(f.path())
        .unwrap()
        .remote_heads("origin")
        .unwrap()
        .into_iter()
        .map(|(name, oid)| format!("{oid}\t{name}"))
        .collect();
    assert_eq!(ours, git);
}

// Past 20 000 lines the journal's record keeps the first 2 000 and the last 5 000, and the
// probe read the heads out of that record: an upstream in the middle was not on the server.
#[test]
fn an_upstream_among_twenty_thousand_heads_is_found() {
    let f = test_fixtures::with_remote().unwrap();
    let url = f
        .git(&["remote", "get-url", "origin"])
        .unwrap()
        .trim()
        .to_owned();
    let server = std::path::Path::new(&url);
    let tip = f
        .git_in(server, &["rev-parse", "refs/heads/main"])
        .unwrap()
        .trim()
        .to_owned();
    let mut packed = String::from("# pack-refs with: peeled fully-peeled sorted \n");
    for side in ["a", "z"] {
        for i in 0..10_000 {
            packed.push_str(&format!("{tip} refs/heads/{side}{i:05}\n"));
        }
    }
    std::fs::write(server.join("packed-refs"), packed).unwrap();

    assert_eq!(git_says(&f), Some(true));
    assert_eq!(
        RepoHandle::open(f.path()).unwrap().pull_probe().unwrap(),
        Some(true)
    );
}

#[test]
fn a_remote_that_is_gone_is_an_error_not_an_answer() {
    let f = test_fixtures::linear(2).unwrap();
    let gone = f.path().join("no-such-remote.git");
    f.git(&["remote", "add", "origin", &gone.to_string_lossy()])
        .unwrap();
    f.git(&["config", "branch.main.remote", "origin"]).unwrap();
    f.git(&["config", "branch.main.merge", "refs/heads/main"])
        .unwrap();
    assert!(RepoHandle::open(f.path()).unwrap().pull_probe().is_err());
}

/// A measurement against the local bare remote, run by hand with `--run-ignored only
/// --no-capture`: the probe, a plain `git ls-remote`, and the fetch it replaces.
#[test]
#[ignore = "a measurement"]
#[allow(clippy::print_stderr)]
fn the_probe_against_ls_remote_and_fetch_takes() {
    let f = test_fixtures::with_remote().unwrap();
    let handle = RepoHandle::open(f.path()).unwrap();
    let median = |mut work: Box<dyn FnMut()>| {
        let mut times: Vec<u128> = (0..11)
            .map(|_| {
                let started = std::time::Instant::now();
                work();
                started.elapsed().as_micros()
            })
            .collect();
        times.sort_unstable();
        (times[5], times[9])
    };
    let probe = median(Box::new(|| {
        handle.pull_probe().unwrap();
    }));
    let ls = median(Box::new(|| {
        f.git(&["ls-remote", "--heads", "origin"]).unwrap();
    }));
    let fetch = median(Box::new(|| {
        handle.background_fetch().unwrap();
        let _ = pulse(f.path());
    }));
    eprintln!("median/p90 µs: probe {probe:?}, git ls-remote {ls:?}, fetch+pulse {fetch:?}");
}
