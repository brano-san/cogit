// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use std::sync::{Arc, Mutex};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

type Lines = Arc<Mutex<Vec<String>>>;

fn collector() -> (Lines, impl FnMut(&str)) {
    let lines: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    (lines, move |line: &str| {
        if let Ok(mut entries) = sink.lock() {
            entries.push(line.to_owned());
        }
    })
}

fn seen(lines: &Lines) -> Vec<String> {
    lines.lock().map(|l| l.clone()).unwrap_or_default()
}

#[test]
fn fetch_brings_the_remote_tracking_branch_up_to_date() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.fetch("origin", None, on_line).unwrap();

    assert!(
        repo.branches()
            .unwrap()
            .iter()
            .any(|b| b.name == "origin/main"),
        "the tracking branch must come back"
    );
}

#[test]
fn fetch_reports_what_git_says_while_it_runs() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    let repo = open(&f);
    let (lines, on_line) = collector();

    repo.fetch("origin", None, on_line).unwrap();

    assert!(
        seen(&lines).iter().any(|l| !l.trim().is_empty()),
        "a fetch that moves refs always says something"
    );
}

#[test]
fn fetching_an_unknown_remote_reports_gits_own_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.fetch("nowhere", None, on_line).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("nowhere"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn push_sends_local_commits_to_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.commit_file(40, "to-push.txt", "pushed\n").unwrap();
    let repo = open(&f);
    let local = f.oid("HEAD").unwrap();
    let (_, on_line) = collector();

    repo.push("origin", None, false, None, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn a_rejected_push_reports_the_reason_in_full() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.push("origin", None, false, None, on_line).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(
                details.stderr.contains("reject") || details.stderr.contains("fetch first"),
                "INV-05: the user needs Git's exact reason, got {:?}",
                details.stderr
            );
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn a_forced_push_overwrites_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let local = f.oid("HEAD").unwrap();
    let (_, on_line) = collector();

    repo.push("origin", None, true, None, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn pull_fast_forwards_when_nothing_is_local() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main~1"]).unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.pull("origin", true, None, on_line).unwrap();

    assert_eq!(
        f.oid("HEAD").unwrap(),
        f.oid("refs/remotes/origin/main").unwrap()
    );
}

#[test]
fn a_pull_that_cannot_fast_forward_is_refused_rather_than_merging_silently() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.pull("origin", true, None, on_line);

    assert!(
        err.is_err(),
        "diverged history must not be merged behind the user's back"
    );
}

#[test]
fn the_remote_list_is_read_without_spawning_a_process() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    let remotes = repo.remotes().unwrap();

    assert_eq!(remotes, vec!["origin".to_owned()]);
    assert!(seen(&log).is_empty());
}

#[test]
fn a_token_reaches_git_as_a_header_but_not_the_journal() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.fetch("origin", Some("s3cr3t"), |_| {}).unwrap();

    let lines = seen(&log).join("\n");
    assert!(lines.contains("http.extraHeader="), "{lines}");
    assert!(!lines.contains("s3cr3t"), "{lines}");
    assert!(
        !lines.contains(&git_engine::auth_header("s3cr3t")),
        "{lines}"
    );
}

#[test]
fn no_token_means_no_extra_argument() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.fetch("origin", None, |_| {}).unwrap();

    assert!(!seen(&log).join("\n").contains("http.extraHeader"));
}
