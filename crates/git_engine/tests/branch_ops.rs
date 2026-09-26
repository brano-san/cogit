#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Renaming, upstream and deleting a remote branch (M5 T5.4). Everything here writes, so
//! everything goes through the system `git`: hooks and server-side refusals are the point.

use git_engine::{RemoteDeletion, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn names(repo: &RepoHandle) -> Vec<String> {
    let mut found: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .filter(|branch| branch.kind == git_engine::BranchKind::Local)
        .map(|branch| branch.name)
        .collect();
    found.sort();
    found
}

#[test]
fn renaming_a_branch_replaces_the_old_name() {
    let f = test_fixtures::branched().unwrap();
    let repo = open(&f);
    repo.rename_branch("dev", "topic", false).unwrap();

    let found = names(&repo);
    assert!(found.contains(&"topic".to_owned()), "{found:?}");
    assert!(!found.contains(&"dev".to_owned()), "{found:?}");
}

#[test]
fn renaming_the_checked_out_branch_keeps_head_on_it() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    let before = repo.head().unwrap();
    repo.rename_branch("main", "trunk", false).unwrap();

    let git_engine::Head::Branch { name, .. } = repo.head().unwrap() else {
        panic!("expected HEAD on a branch, was {before:?}");
    };
    assert_eq!(name, "trunk");
}

#[test]
fn renaming_onto_a_taken_name_is_refused_in_gits_words() {
    let f = test_fixtures::branched().unwrap();
    let err = open(&f).rename_branch("dev", "main", false).unwrap_err();
    assert!(format!("{err:?}").contains("already exists"), "{err:?}");
}

#[test]
fn renaming_onto_a_taken_name_succeeds_when_forced() {
    let f = test_fixtures::branched().unwrap();
    // Not onto `main`: git refuses to force-update the branch a worktree has checked out.
    f.git(&["branch", "spare"]).unwrap();
    let repo = open(&f);

    repo.rename_branch("dev", "spare", true).unwrap();
    let found = names(&repo);
    assert!(!found.contains(&"dev".to_owned()), "{found:?}");
    assert!(found.contains(&"spare".to_owned()), "{found:?}");
}

#[test]
fn an_empty_new_name_is_refused_before_git_is_spawned() {
    let f = test_fixtures::branched().unwrap();
    assert!(open(&f).rename_branch("dev", "  ", false).is_err());
}

/// Re-opened on purpose: a handle holds the config as it was when it was opened, and
/// `set_upstream` writes through the CLI (R-53).
fn upstream_of(f: &test_fixtures::Fixture, branch: &str) -> Option<String> {
    open(f)
        .branches()
        .unwrap()
        .into_iter()
        .find(|found| found.name == branch)
        .and_then(|found| found.upstream)
}

#[test]
fn setting_an_upstream_makes_the_branch_track_it() {
    let f = test_fixtures::with_remote().unwrap();
    // `main` already tracks in this fixture, so a fresh branch is the only honest subject.
    f.git(&["branch", "solo"]).unwrap();
    assert_eq!(upstream_of(&f, "solo"), None);

    open(&f).set_upstream("solo", Some("origin/main")).unwrap();
    assert_eq!(upstream_of(&f, "solo").as_deref(), Some("origin/main"));
}

#[test]
fn clearing_the_upstream_leaves_the_branch_untracked() {
    let f = test_fixtures::with_remote().unwrap();
    assert_eq!(upstream_of(&f, "main").as_deref(), Some("origin/main"));

    open(&f).set_upstream("main", None).unwrap();
    assert_eq!(upstream_of(&f, "main"), None);
}

#[test]
fn a_handle_opened_before_the_write_still_reports_the_old_upstream() {
    let f = test_fixtures::with_remote().unwrap();
    let stale = open(&f);
    stale.set_upstream("main", None).unwrap();

    // Not a wish, a warning: `AppState::handle` re-opens per call for exactly this reason.
    let seen = stale
        .branches()
        .unwrap()
        .into_iter()
        .find(|branch| branch.name == "main")
        .and_then(|branch| branch.upstream);
    assert_eq!(
        seen.as_deref(),
        Some("origin/main"),
        "the config snapshot predates the write (R-53)"
    );
    assert_eq!(upstream_of(&f, "main"), None);
}

#[test]
fn an_upstream_that_does_not_exist_is_refused() {
    let f = test_fixtures::with_remote().unwrap();
    assert!(open(&f).set_upstream("main", Some("origin/nope")).is_err());
}

#[test]
fn deleting_a_remote_branch_removes_it_from_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    f.git(&["push", "origin", "HEAD:refs/heads/doomed"])
        .unwrap();

    assert_eq!(
        repo.delete_remote_branch("origin", "doomed").unwrap(),
        RemoteDeletion::Deleted
    );
    f.git(&["fetch", "--prune", "origin"]).unwrap();

    let remote: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .filter(|branch| branch.kind == git_engine::BranchKind::Remote)
        .map(|branch| branch.name)
        .collect();
    assert!(
        !remote.iter().any(|name| name.ends_with("doomed")),
        "{remote:?}"
    );
}

// A branch someone else already deleted is what the user asked for, not a failure; the
// stale remote-tracking ref that still showed it goes, as `fetch --prune` would drop it.
#[test]
fn deleting_a_remote_branch_that_is_already_gone_succeeds() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "refs/remotes/origin/gone", "HEAD"])
        .unwrap();

    let result = open(&f)
        .delete_remote_branch("origin", "origin/gone")
        .unwrap();

    assert_eq!(result, RemoteDeletion::AlreadyGone);
    assert!(
        f.git(&["rev-parse", "--verify", "-q", "refs/remotes/origin/gone"])
            .is_err()
    );
}

// With a tag of the same name on the server, `push --delete origin topic` refused:
// "dst refspec topic matches more than one".
#[test]
fn a_remote_branch_with_a_tag_of_the_same_name_is_deleted_and_the_tag_kept() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&[
        "push",
        "origin",
        "HEAD:refs/heads/topic",
        "HEAD:refs/tags/topic",
    ])
    .unwrap();

    let result = open(&f)
        .delete_remote_branch("origin", "origin/topic")
        .unwrap();

    assert_eq!(result, RemoteDeletion::Deleted);
    let left = f.git(&["ls-remote", "origin"]).unwrap();
    assert!(!left.contains("refs/heads/topic"), "{left}");
    assert!(left.contains("refs/tags/topic"), "{left}");
}

#[test]
fn the_server_branch_is_found_through_the_remotes_fetch_refspec() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&[
        "config",
        "remote.origin.fetch",
        "+refs/heads/*:refs/remotes/origin/mirror/*",
    ])
    .unwrap();
    f.git(&["push", "origin", "HEAD:refs/heads/feature"])
        .unwrap();
    f.git(&["fetch", "origin"]).unwrap();

    let result = open(&f)
        .delete_remote_branch("origin", "origin/mirror/feature")
        .unwrap();

    assert_eq!(result, RemoteDeletion::Deleted);
    let left = f.git(&["ls-remote", "--heads", "origin"]).unwrap();
    assert!(!left.contains("refs/heads/feature"), "{left}");
}

#[test]
fn a_remote_branch_is_named_without_its_remote_prefix() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    f.git(&["push", "origin", "HEAD:refs/heads/short"]).unwrap();

    // `origin/short` is what the panel shows; the command has to take either spelling.
    repo.delete_remote_branch("origin", "origin/short").unwrap();
    f.git(&["fetch", "--prune", "origin"]).unwrap();
    assert!(
        !repo
            .branches()
            .unwrap()
            .iter()
            .any(|branch| branch.name.ends_with("short"))
    );
}
