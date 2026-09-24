// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn write_graph(f: &test_fixtures::Fixture) {
    f.git(&["commit-graph", "write", "--reachable"]).unwrap();
}

/// `origin/main` is one remote commit past the shared base; HEAD is two local commits past it.
fn diverged() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::with_remote().unwrap();
    let base = f.oid("HEAD~2").unwrap();
    (f, base)
}

#[test]
fn the_commit_graph_answers_without_the_slow_walk() {
    let (f, base) = diverged();
    write_graph(&f);
    let repo = open(&f);

    assert_eq!(repo.published_in_process(&base), Some(true));
    assert_eq!(repo.published_in_process("HEAD"), Some(false));
    assert_eq!(repo.published_in_process("HEAD~1"), Some(false));
}

#[test]
fn without_a_commit_graph_the_answer_comes_from_git() {
    let (f, base) = diverged();
    let repo = open(&f);

    assert_eq!(repo.published_in_process(&base), None);
    assert!(repo.is_published(&base).unwrap());
    assert!(!repo.is_published("HEAD").unwrap());
}

#[test]
fn a_commit_made_after_the_graph_was_written_is_still_judged() {
    let (f, _) = diverged();
    write_graph(&f);
    f.commit_file(40, "later.txt", "later\n").unwrap();
    let repo = open(&f);

    assert_eq!(repo.published_in_process("HEAD"), Some(false));
    assert!(!repo.is_published("HEAD").unwrap());
}

#[test]
fn a_remote_that_moved_on_after_the_graph_was_written_still_counts() {
    let (f, _) = diverged();
    write_graph(&f);
    f.commit_file(40, "pushed.txt", "pushed\n").unwrap();
    let pushed = f.oid("HEAD").unwrap();
    f.git(&["push", "origin", "HEAD:refs/heads/topic"]).unwrap();
    f.git(&["fetch", "origin"]).unwrap();
    let repo = open(&f);

    assert_eq!(repo.published_in_process(&pushed), Some(true));
    assert_eq!(repo.published_in_process("HEAD~1"), Some(true));
}

#[test]
fn an_old_local_commit_beside_the_remote_is_not_published() {
    let (f, base) = diverged();
    f.git(&["branch", "side", &base]).unwrap();
    f.git(&["checkout", "-q", "side"]).unwrap();
    f.commit_file(41, "side.txt", "side\n").unwrap();
    let side = f.oid("HEAD").unwrap();
    write_graph(&f);
    let repo = open(&f);

    assert_eq!(repo.published_in_process(&side), Some(false));
}

#[test]
fn an_unknown_revision_is_left_to_git_to_refuse() {
    let (f, _) = diverged();
    write_graph(&f);
    let repo = open(&f);

    assert_eq!(repo.published_in_process("no-such-rev"), None);
    assert!(repo.is_published("no-such-rev").is_err());
}

#[test]
fn without_remote_branches_nothing_needs_walking() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert_eq!(repo.published_in_process("HEAD"), Some(false));
    assert_eq!(repo.published_in_process("no-such-rev"), None);
}
