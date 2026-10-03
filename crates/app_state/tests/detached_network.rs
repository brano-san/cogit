// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Pull and Push on a detached HEAD (F-710): Pull only fetches, Push sends every branch
//! ahead of its upstream and no tag.

use app_state::{AppState, RepoId};
use git_engine::{NetworkStop, PullOutcome, Pushed};

/// `main` has diverged from `origin/main`; `topic` is one commit ahead of `origin/topic`,
/// `synced` is level with its upstream, `solo` tracks nothing. `v1` is on the remote,
/// `v2` is not. HEAD is detached.
fn detached() -> (test_fixtures::Fixture, AppState, RepoId) {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "push.default", "simple"]).unwrap();
    f.git(&["switch", "-c", "synced", "origin/main"]).unwrap();
    f.git(&["push", "-u", "origin", "synced"]).unwrap();
    f.git(&["switch", "-c", "topic"]).unwrap();
    f.git(&["push", "-u", "origin", "topic"]).unwrap();
    f.commit_file(50, "topic.txt", "topic\n").unwrap();
    f.git(&["tag", "v1"]).unwrap();
    f.git(&["push", "origin", "v1"]).unwrap();
    f.git(&["switch", "-c", "solo"]).unwrap();
    f.commit_file(51, "solo.txt", "solo\n").unwrap();
    f.git(&["tag", "v2"]).unwrap();
    f.git(&["switch", "--detach", "HEAD~1"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (f, state, repo)
}

fn on_remote(f: &test_fixtures::Fixture, name: &str) -> Option<String> {
    let listed = f.git(&["ls-remote", "origin", name]).unwrap();
    listed.split_whitespace().next().map(str::to_owned)
}

#[test]
fn push_sends_the_branches_ahead_of_their_upstream_and_no_tags() {
    let (f, state, repo) = detached();

    let pushed = state
        .push(repo, "origin", false, &NetworkStop::default(), |_| {})
        .unwrap();

    assert_eq!(
        pushed,
        Some(Pushed {
            branches: vec!["topic".to_owned()],
        })
    );
    assert_eq!(
        on_remote(&f, "refs/heads/topic"),
        Some(f.oid("topic").unwrap())
    );
    assert_eq!(on_remote(&f, "refs/tags/v2"), None);
    // Diverged: only a forced push could send it, and none is made.
    assert_ne!(
        on_remote(&f, "refs/heads/main"),
        Some(f.oid("main").unwrap())
    );
    assert_eq!(on_remote(&f, "refs/heads/solo"), None);
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim(),
        "HEAD"
    );
}

#[test]
fn push_with_nothing_pushable_runs_nothing_and_says_so() {
    let (f, state, repo) = detached();
    state
        .push(repo, "origin", false, &NetworkStop::default(), |_| {})
        .unwrap();
    let before = f.git(&["ls-remote", "origin"]).unwrap();

    let again = state
        .push(repo, "origin", false, &NetworkStop::default(), |_| {})
        .unwrap();

    assert_eq!(again, Some(Pushed::default()));
    assert_eq!(f.git(&["ls-remote", "origin"]).unwrap(), before);
}

#[test]
fn pull_on_a_detached_head_only_fetches() {
    let (f, state, repo) = detached();
    let head = f.oid("HEAD").unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();

    let outcome = state
        .pull(repo, "origin", false, &NetworkStop::default(), |_| {})
        .unwrap();

    assert_eq!(outcome, PullOutcome::FetchedDetached);
    assert_eq!(f.oid("HEAD").unwrap(), head);
    assert!(f.oid("refs/remotes/origin/main").is_ok());
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim(),
        "HEAD"
    );
}

#[test]
fn pull_on_a_branch_still_pulls() {
    let (f, state, repo) = detached();
    f.git(&["switch", "synced"]).unwrap();

    let outcome = state
        .pull(repo, "origin", true, &NetworkStop::default(), |_| {})
        .unwrap();

    assert_eq!(outcome, PullOutcome::Pulled);
}
