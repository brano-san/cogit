// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, SubmoduleState};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_repository_without_submodules_lists_none() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).submodules().unwrap().is_empty());
}

#[test]
fn a_submodule_is_listed_with_its_path_and_url() {
    let f = test_fixtures::with_submodule().unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].path, "vendor/lib");
    assert!(!modules[0].url.is_empty());
}

#[test]
fn a_checked_out_submodule_is_in_sync() {
    let f = test_fixtures::with_submodule().unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::InSync);
    assert_eq!(modules[0].recorded.len(), 40);
}

#[test]
fn an_uninitialised_submodule_is_reported_as_such() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::NotInitialised);
    assert!(modules[0].checked_out.is_none());
}

/// Commits in the submodule the parent has never been told about. The inner repository
/// has no identity of its own, so the commit brings one.
fn commit_inside(f: &test_fixtures::Fixture, inner: &std::path::Path, message: &str) {
    f.git_in(
        inner,
        &[
            "-c",
            "user.name=Cogit Test",
            "-c",
            "user.email=test@cogit.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    )
    .unwrap();
}

/// Requirement 6: three different situations needing three different actions, not one
/// "diverged" for all of them (doc/12-risks.md, R-153).
#[test]
fn a_submodule_checked_out_on_an_older_commit_is_behind() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    let before = f.git_in(&inner, &["rev-parse", "HEAD"]).unwrap();
    f.git_in(&inner, &["checkout", "--detach", "HEAD~1"])
        .unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::Behind);
    assert_eq!((modules[0].ahead, modules[0].behind), (0, 1));
    assert_eq!(modules[0].recorded, before.trim());
}

#[test]
fn a_submodule_with_new_commits_on_top_is_ahead() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    commit_inside(&f, &inner, "one more");

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::Ahead);
    assert_eq!((modules[0].ahead, modules[0].behind), (1, 0));
}

#[test]
fn a_submodule_on_a_line_of_its_own_has_diverged() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    f.git_in(&inner, &["checkout", "--detach", "HEAD~1"])
        .unwrap();
    commit_inside(&f, &inner, "elsewhere");

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::Diverged);
    assert_eq!((modules[0].ahead, modules[0].behind), (1, 1));
}

/// "Do not show an inexact label as an exact one": a recorded commit the submodule has
/// never fetched cannot be placed, and saying "diverged" would be a guess.
#[test]
fn a_recorded_commit_the_submodule_does_not_have_is_not_guessed_at() {
    let f = test_fixtures::with_submodule().unwrap();
    let absent = "1234567890abcdef1234567890abcdef12345678";
    f.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{absent},vendor/lib"),
    ])
    .unwrap();
    f.commit_staged(2, "record a commit nobody fetched")
        .unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].recorded, absent);
    assert_eq!(modules[0].state, SubmoduleState::Unknown);
}

/// Added and not committed yet: the gitlink is only in the index, where `git submodule
/// status` reads it too. Read from HEAD alone it was "", which no commit matches.
#[test]
fn a_submodule_added_but_not_committed_is_in_step_with_its_staged_pointer() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["reset", "-q", "--soft", "HEAD~1"]).unwrap();

    let module = open(&f).submodules().unwrap().remove(0);

    assert_eq!(module.state, SubmoduleState::InSync, "{module:?}");
    assert_eq!(Some(&module.recorded), module.checked_out.as_ref());
}

/// Still in `.gitmodules`, but no gitlink in HEAD or the index: nothing is recorded, and
/// that is not a commit the submodule is missing.
#[test]
fn a_submodule_neither_head_nor_the_index_records_says_so() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["rm", "-q", "--cached", "vendor/lib"]).unwrap();
    f.commit_staged(2, "stop recording vendor/lib").unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules.len(), 1, "still listed in .gitmodules: {modules:?}");
    assert_eq!(modules[0].state, SubmoduleState::Unrecorded);
    assert!(modules[0].recorded.is_empty());
}

#[test]
fn the_submodule_can_be_opened_as_a_repository_of_its_own() {
    let f = test_fixtures::with_submodule().unwrap();
    let modules = open(&f).submodules().unwrap();

    let inner = RepoHandle::open(&f.path().join(&modules[0].path)).unwrap();

    assert!(inner.head().is_ok());
}

#[test]
fn listing_submodules_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::with_submodule().unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.submodules().unwrap();

    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn initialising_an_uninitialised_submodule_brings_it_back() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();
    let repo = open(&f);

    repo.update_submodule("vendor/lib", true).unwrap();

    assert_eq!(repo.submodules().unwrap()[0].state, SubmoduleState::InSync);
}

#[test]
fn updating_a_submodule_left_behind_puts_it_back_on_the_recorded_commit() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    f.git_in(&inner, &["checkout", "--detach", "HEAD~1"])
        .unwrap();
    let repo = open(&f);

    repo.update_submodule("vendor/lib", false).unwrap();

    assert_eq!(repo.submodules().unwrap()[0].state, SubmoduleState::InSync);
}

#[test]
fn an_unknown_submodule_path_is_a_typed_error() {
    let f = test_fixtures::with_submodule().unwrap();
    assert!(open(&f).update_submodule("no/such/module", false).is_err());
}

#[test]
fn a_submodule_inside_a_submodule_is_reachable() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let top = RepoHandle::open(f.path()).unwrap();

    let outer = top.submodules().unwrap();
    assert_eq!(outer.len(), 1, "{outer:?}");
    assert_eq!(outer[0].path, "vendor/middle");

    let inner = RepoHandle::open(&f.path().join("vendor").join("middle"))
        .unwrap()
        .submodules()
        .unwrap();
    assert_eq!(inner.len(), 1, "{inner:?}");
    assert_eq!(inner[0].path, "deep/inner");
}

#[test]
fn the_second_level_is_a_repository_in_its_own_right() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let deep = f
        .path()
        .join("vendor")
        .join("middle")
        .join("deep")
        .join("inner");

    assert!(RepoHandle::open(&deep).is_ok(), "{deep:?}");
}

#[test]
fn a_path_that_is_not_there_is_an_error_rather_than_a_panic() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(RepoHandle::open(&f.path().join("no-such-folder")).is_err());
}

#[test]
fn a_folder_that_is_not_a_repository_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(RepoHandle::open(dir.path()).is_err());
}

/// The row in the tree has to say what the submodule is on, and a submodule checked out
/// by `git submodule update` is detached, so the branch alone is never enough.
#[test]
fn a_submodule_reports_the_commit_it_sits_on_and_its_subject() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "update",
        "--init",
    ])
    .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let module = repo.submodules().unwrap().into_iter().next().unwrap();

    assert!(module.checked_out.is_some(), "{module:?}");
    assert!(
        module
            .subject
            .as_deref()
            .is_some_and(|line| !line.is_empty()),
        "the row needs something to show beside the hash: {module:?}"
    );
}

#[test]
fn an_uninitialised_submodule_reports_no_branch_and_no_subject() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let module = repo.submodules().unwrap().into_iter().next().unwrap();

    assert_eq!(module.state, git_engine::SubmoduleState::NotInitialised);
    assert_eq!(module.branch, None);
    assert_eq!(module.subject, None);
}

/// The Repositories tree draws a disclosure triangle only where there is something to
/// open. "Not known yet" must not draw one that later disappears (doc/12-risks.md, R-148).
#[test]
fn a_submodule_says_whether_it_holds_submodules_of_its_own() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let modules = open(&f).submodules().unwrap();
    let outer = modules.first().expect("the parent has one submodule");
    assert!(
        outer.nested,
        "{} holds deep/inner and should say so",
        outer.path
    );
}

#[test]
fn a_leaf_submodule_does_not_claim_to_hold_more() {
    let f = test_fixtures::with_submodule().unwrap();
    let modules = open(&f).submodules().unwrap();
    assert!(!modules.first().expect("one submodule").nested);
}

#[test]
fn an_uninitialised_submodule_holds_nothing_anyone_can_see() {
    let f = test_fixtures::with_submodule().unwrap();
    let modules = open(&f).submodules().unwrap();
    let path = f.path().join(&modules.first().expect("one submodule").path);
    std::fs::remove_dir_all(&path).unwrap();
    assert!(!open(&f).submodules().unwrap().first().unwrap().nested);
}

/// The Repositories tree marks a submodule stopped half way through a merge, as it marks
/// a repository (#22).
#[test]
fn a_submodule_reports_an_operation_stopped_inside_it() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    let git_dir = f
        .git_in(&inner, &["rev-parse", "--absolute-git-dir"])
        .unwrap();
    std::fs::write(
        std::path::Path::new(git_dir.trim()).join("MERGE_HEAD"),
        "0".repeat(40),
    )
    .unwrap();

    let module = open(&f).submodules().unwrap().into_iter().next().unwrap();

    assert_eq!(module.repo_state, Some(git_engine::RepoState::Merging));
}

#[test]
fn an_uninitialised_submodule_has_no_repository_state() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();

    let module = open(&f).submodules().unwrap().into_iter().next().unwrap();

    assert_eq!(module.repo_state, None);
}

// The tree of a repository that is not on screen (R-352): `.gitmodules` and the gitlinks,
// nothing read from inside the submodules.
#[test]
fn an_outline_lists_a_submodule_without_looking_inside_it() {
    let f = test_fixtures::with_submodule().unwrap();

    let modules = open(&f).submodule_outline().unwrap();

    assert_eq!(modules.len(), 1);
    let module = &modules[0];
    assert_eq!(module.path, "vendor/lib");
    assert!(!module.url.is_empty());
    assert_eq!(module.recorded, f.oid("HEAD:vendor/lib").unwrap());
    assert_eq!(module.state, SubmoduleState::Unread);
    assert!(module.checked_out.is_none());
    assert!(module.branch.is_none() && module.subject.is_none());
    assert!(!module.nested);
}

#[test]
fn an_outline_tells_an_uninitialised_submodule() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();

    let modules = open(&f).submodule_outline().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::NotInitialised);
}

#[test]
fn an_outline_knows_which_submodules_have_their_own() {
    let f = test_fixtures::with_nested_submodule().unwrap();

    let top = open(&f).submodule_outline().unwrap();
    assert!(top[0].nested);

    let middle = RepoHandle::open_exact(&f.path().join("vendor/middle")).unwrap();
    let deep = middle.submodule_outline().unwrap();
    assert_eq!(deep[0].path, "deep/inner");
    assert!(!deep[0].nested);
}

#[test]
fn an_outline_of_a_repository_without_submodules_is_empty() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).submodule_outline().unwrap().is_empty());
}
