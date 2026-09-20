// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CommitRequest, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn request(message: &str, no_verify: bool) -> CommitRequest {
    CommitRequest {
        message: message.to_owned(),
        amend: false,
        no_verify,
        only: Vec::new(),
    }
}

fn stage_something(f: &test_fixtures::Fixture, name: &str) {
    std::fs::write(f.path().join(name), "content\n").unwrap();
    f.git(&["add", "--", name]).unwrap();
}

#[test]
fn a_fresh_repository_has_bypassed_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).bypass_log().unwrap().is_empty());
}

#[test]
fn committing_with_no_verify_is_recorded() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    open(&f).commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(open(&f).bypass_log().unwrap().len(), 1);
}

#[test]
fn an_ordinary_commit_is_not_recorded_as_a_bypass() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    open(&f).commit(&request("run the hooks", false)).unwrap();

    assert!(open(&f).bypass_log().unwrap().is_empty());
}

#[test]
fn the_record_names_the_commit_it_belongs_to() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);

    let oid = repo.commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(repo.bypass_log().unwrap()[0].oid, oid);
}

#[test]
fn the_record_keeps_the_subject_so_the_summary_reads_sensibly() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);

    repo.commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(repo.bypass_log().unwrap()[0].summary, "skip the hooks");
}

#[test]
fn several_bypasses_accumulate_newest_first() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    for name in ["a.txt", "b.txt"] {
        stage_something(&f, name);
        repo.commit(&request(&format!("bypass for {name}"), true))
            .unwrap();
    }

    let log = repo.bypass_log().unwrap();

    assert_eq!(log.len(), 2);
    assert_eq!(log[0].summary, "bypass for b.txt", "{log:?}");
}

#[test]
fn a_corrupt_record_is_skipped_rather_than_failing_the_read() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);
    repo.commit(&request("skip the hooks", true)).unwrap();

    let path = f.path().join(".git/cogit-hook-bypasses");
    let good = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("nonsense without separators\n{good}")).unwrap();

    assert_eq!(repo.bypass_log().unwrap().len(), 1);
}

#[test]
fn a_commit_that_fails_is_not_recorded_as_a_bypass() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.commit(&request("nothing is staged", true)).is_err());
    assert!(repo.bypass_log().unwrap().is_empty());
}
