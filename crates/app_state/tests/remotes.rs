// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Pull, push and fetch as the toolbar and the Branches menus run them: through the
//! state's cached handles, on a repository opened before the branch existed.

use app_state::{AppState, RepoId};
use git_engine::{Branch, BranchKind, NetworkStop};

fn branch(state: &AppState, repo: RepoId, kind: BranchKind, name: &str) -> Option<Branch> {
    state
        .repo_refs(repo)
        .unwrap()
        .branches
        .into_iter()
        .find(|branch| branch.kind == kind && branch.name == name)
}

/// Opened, and read once, before the branch is made: the handle the state keeps is
/// older than the branch and its config.
fn with_new_branch(name: &str) -> (test_fixtures::Fixture, AppState, RepoId) {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "push.default", "simple"]).unwrap();
    f.git(&["config", "push.autoSetupRemote", "false"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let _ = state.repo_refs(repo).unwrap();
    f.git(&["switch", "-c", name]).unwrap();
    f.commit_file(40, "feature.txt", "new\n").unwrap();
    let _ = state.repo_refs(repo).unwrap();
    (f, state, repo)
}

/// What the graph joins into one `origin=<name>` label: the local branch tracks the
/// remote branch of the same name, and both are at one commit (F-330).
fn joined(state: &AppState, repo: RepoId, name: &str) -> bool {
    let local = branch(state, repo, BranchKind::Local, name).unwrap();
    let remote = branch(state, repo, BranchKind::Remote, &format!("origin/{name}"));
    local.upstream.as_deref() == Some(&format!("origin/{name}"))
        && remote.is_some_and(|remote| remote.oid == local.oid)
}

// The toolbar's Push of a branch never pushed: it went out, "The current branch … has no
// upstream branch", in a build older than R-414.
#[test]
fn a_new_branch_pushed_from_the_toolbar_tracks_its_remote_branch() {
    let (_f, state, repo) = with_new_branch("audit/2026-09-25");

    state
        .push(repo, "origin", false, &NetworkStop::default(), |_| {})
        .unwrap();

    assert!(joined(&state, repo, "audit/2026-09-25"));
}

// Push in the menu of the branch sent `refs/heads/x:refs/heads/x` without --set-upstream:
// the graph showed `x` and `origin/x` side by side, since the branch tracked nothing.
#[test]
fn a_new_branch_pushed_from_its_menu_is_joined_with_its_remote_branch() {
    let (_f, state, repo) = with_new_branch("audit/2026-09-25");
    let refspec = "refs/heads/audit/2026-09-25:refs/heads/audit/2026-09-25";

    state
        .push_to(
            repo,
            "origin",
            refspec,
            true,
            &NetworkStop::default(),
            |_| {},
        )
        .unwrap();

    assert!(joined(&state, repo, "audit/2026-09-25"));
}

#[test]
fn a_ref_pushed_without_tracking_leaves_the_branch_as_it_was() {
    let (_f, state, repo) = with_new_branch("topic");

    state
        .push_to(
            repo,
            "origin",
            "refs/heads/topic:refs/heads/review/topic",
            false,
            &NetworkStop::default(),
            |_| {},
        )
        .unwrap();

    assert_eq!(
        branch(&state, repo, BranchKind::Local, "topic")
            .unwrap()
            .upstream,
        None
    );
}

/// A second remote, `upstream`, with a branch of its own the local repository has not
/// fetched yet; `origin/main` is deleted locally, so a fetch of origin brings it back.
fn with_two_remotes(f: &test_fixtures::Fixture) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    let bare = dir.path().join("upstream.git");
    let bare = bare.to_string_lossy().replace('\\', "/");
    f.git(&["init", "--bare", "--initial-branch=main", &bare])
        .unwrap();
    f.git(&["remote", "add", "upstream", &bare]).unwrap();
    f.git(&["push", "upstream", "HEAD:refs/heads/release"])
        .unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    dir
}

// Pull on a branch that tracks nothing ended in git's error, "You asked to pull from the
// remote 'origin', but did not specify a branch": it fetches every remote instead.
#[test]
fn a_pull_on_a_branch_without_upstream_fetches_every_remote() {
    let (f, state, repo) = with_new_branch("topic");
    let _upstream = with_two_remotes(&f);
    let head = f.oid("HEAD").unwrap();
    let undo_before = state.safety_log().len();

    state
        .pull(repo, "origin", true, &NetworkStop::default(), |_| {})
        .unwrap();

    assert!(branch(&state, repo, BranchKind::Remote, "origin/main").is_some());
    assert!(branch(&state, repo, BranchKind::Remote, "upstream/release").is_some());
    assert_eq!(f.oid("HEAD").unwrap(), head, "nothing is merged");
    assert_eq!(
        state.safety_log().len(),
        undo_before,
        "a fetch moves no branch, so Undo has nothing to take back"
    );
}
