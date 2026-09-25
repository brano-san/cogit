// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Remote ▸ Submodule (#45).

use git_engine::{GitError, RepoHandle, SubmoduleOp, SubmoduleState};
use std::collections::BTreeSet;

const MODULE: &str = "vendor/lib";

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn state(f: &test_fixtures::Fixture) -> SubmoduleState {
    open(f).submodules().unwrap()[0].state
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|path| (*path).to_owned()).collect()
}

fn url_of(f: &test_fixtures::Fixture) -> String {
    f.path().to_string_lossy().replace('\\', "/")
}

fn deinit(f: &test_fixtures::Fixture) {
    f.git(&["submodule", "deinit", "-f", "--", MODULE]).unwrap();
}

fn local_config(f: &test_fixtures::Fixture, key: &str) -> Option<String> {
    f.git(&["config", "--local", "--get", key])
        .ok()
        .map(|value| value.trim().to_owned())
}

#[test]
fn initialize_checks_out_every_submodule_when_none_is_named() {
    let f = test_fixtures::with_submodule().unwrap();
    deinit(&f);

    open(&f).submodule_op(SubmoduleOp::Initialize, &[]).unwrap();

    assert_eq!(state(&f), SubmoduleState::InSync);
}

#[test]
fn a_path_that_is_not_a_submodule_is_refused_before_git_runs() {
    let f = test_fixtures::with_submodule().unwrap();

    let refused = open(&f).submodule_op(SubmoduleOp::Initialize, &paths(&["README.md"]));

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "{refused:?}"
    );
}

#[test]
fn synchronize_copies_the_url_from_gitmodules_into_the_config() {
    let f = test_fixtures::with_submodule().unwrap();
    let key = format!("submodule.{MODULE}.url");
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        &key,
        "https://example.invalid/moved.git",
    ])
    .unwrap();

    open(&f)
        .submodule_op(SubmoduleOp::Synchronize, &paths(&[MODULE]))
        .unwrap();

    assert_eq!(
        local_config(&f, &key).as_deref(),
        Some("https://example.invalid/moved.git")
    );
}

#[test]
fn reset_puts_the_submodule_back_on_the_recorded_commit() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join(MODULE);
    f.git_in(&inner, &["checkout", "--detach", "HEAD~1"])
        .unwrap();
    assert_eq!(state(&f), SubmoduleState::Behind);

    open(&f)
        .submodule_op(SubmoduleOp::Reset, &paths(&[MODULE]))
        .unwrap();

    assert_eq!(state(&f), SubmoduleState::InSync);
}

/// Changing the repository behind the user's back needs them to say which submodule.
#[test]
fn the_operations_that_change_a_checkout_need_a_submodule_named() {
    let f = test_fixtures::with_submodule().unwrap();
    for op in [
        SubmoduleOp::Reset,
        SubmoduleOp::Deactivate,
        SubmoduleOp::Deinit,
        SubmoduleOp::Unregister,
    ] {
        let refused = open(&f).submodule_op(op, &[]);
        assert!(
            matches!(refused, Err(GitError::InvalidState(_))),
            "{op:?}: {refused:?}"
        );
    }
}

#[test]
fn add_registers_and_checks_out_a_new_submodule() {
    let f = test_fixtures::with_submodule().unwrap();
    let source = test_fixtures::linear(2).unwrap();

    open(&f)
        .add_submodule(&url_of(&source), "libs/other", None)
        .unwrap();

    let modules = open(&f).submodules().unwrap();
    let added = modules.iter().find(|module| module.path == "libs/other");
    assert!(
        added.is_some_and(|module| module.checked_out.is_some()),
        "{modules:?}"
    );
    let staged = f.git(&["diff", "--cached", "--name-only"]).unwrap();
    assert!(
        staged.contains(".gitmodules") && staged.contains("libs/other"),
        "{staged}"
    );
}

#[test]
fn add_with_a_branch_records_the_branch_in_gitmodules() {
    let f = test_fixtures::with_submodule().unwrap();
    let source = test_fixtures::linear(2).unwrap();

    open(&f)
        .add_submodule(&url_of(&source), "libs/other", Some("main"))
        .unwrap();

    let branch = f
        .git(&["config", "-f", ".gitmodules", "submodule.libs/other.branch"])
        .unwrap();
    assert_eq!(branch.trim(), "main");
}

#[test]
fn add_refuses_an_empty_url_or_path() {
    let f = test_fixtures::with_submodule().unwrap();
    assert!(open(&f).add_submodule(" ", "libs/other", None).is_err());
    assert!(
        open(&f)
            .add_submodule("https://example.invalid/x.git", "", None)
            .is_err()
    );
}

#[test]
fn deactivate_marks_the_submodule_inactive_and_initialize_brings_it_back() {
    let f = test_fixtures::with_submodule().unwrap();
    let key = format!("submodule.{MODULE}.active");

    open(&f)
        .submodule_op(SubmoduleOp::Deactivate, &paths(&[MODULE]))
        .unwrap();
    assert_eq!(local_config(&f, &key).as_deref(), Some("false"));

    open(&f)
        .submodule_op(SubmoduleOp::Initialize, &paths(&[MODULE]))
        .unwrap();
    assert_ne!(local_config(&f, &key).as_deref(), Some("false"));
}

#[test]
fn deinit_empties_the_checkout() {
    let f = test_fixtures::with_submodule().unwrap();

    open(&f)
        .submodule_op(SubmoduleOp::Deinit, &paths(&[MODULE]))
        .unwrap();

    assert_eq!(state(&f), SubmoduleState::NotInitialised);
}

/// No `--force`: work inside the submodule is the user's, and git's refusal says so.
#[test]
fn deinit_refuses_a_submodule_with_local_changes() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join(MODULE);
    let tracked = f.git_in(&inner, &["ls-files"]).unwrap();
    let file = tracked.lines().next().unwrap();
    std::fs::write(inner.join(file), "edited inside the submodule\n").unwrap();

    let refused = open(&f).submodule_op(SubmoduleOp::Deinit, &paths(&[MODULE]));

    assert!(matches!(refused, Err(GitError::Command(_))), "{refused:?}");
    assert_eq!(
        std::fs::read_to_string(inner.join(file)).unwrap(),
        "edited inside the submodule\n"
    );
}

#[test]
fn unregister_forgets_the_submodule_but_keeps_its_files() {
    let f = test_fixtures::with_submodule().unwrap();

    open(&f)
        .submodule_op(SubmoduleOp::Unregister, &paths(&[MODULE]))
        .unwrap();

    assert!(open(&f).submodules().unwrap().is_empty());
    assert!(
        f.path().join(MODULE).join(".git").exists(),
        "the checkout was removed"
    );
    let staged = f.git(&["diff", "--cached", "--name-status"]).unwrap();
    assert!(staged.contains(&format!("D\t{MODULE}")), "{staged}");
    assert_eq!(local_config(&f, &format!("submodule.{MODULE}.url")), None);
}

/// Fetch and Pull ▸ Initialize new submodules: only the ones the pull brought, never one
/// the user deinitialised on purpose.
#[test]
fn only_submodules_that_were_not_there_before_are_initialised() {
    let f = test_fixtures::with_submodule().unwrap();
    deinit(&f);

    let known: BTreeSet<String> = [MODULE.to_owned()].into();
    let started = open(&f).init_submodules_added_since(&known).unwrap();
    assert!(started.is_empty());
    assert_eq!(state(&f), SubmoduleState::NotInitialised);

    let started = open(&f)
        .init_submodules_added_since(&BTreeSet::new())
        .unwrap();
    assert_eq!(started, [MODULE.to_owned()]);
    assert_eq!(state(&f), SubmoduleState::InSync);
}

#[test]
fn the_paths_listed_are_the_ones_in_gitmodules() {
    let f = test_fixtures::with_submodule().unwrap();
    let listed = open(&f).submodule_paths();
    assert_eq!(listed, [MODULE.to_owned()].into());
}

fn fresh_clone(f: &test_fixtures::Fixture) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let into = dir.path().join("clone");
    f.git(&["clone", "-q", "--", &url_of(f), &into.to_string_lossy()])
        .unwrap();
    dir
}

fn transport_refused(result: Result<(), GitError>) -> bool {
    matches!(result, Err(GitError::Command(ref failed)) if failed.stderr.contains("transport 'file' not allowed"))
}

// A local path in someone else's `.gitmodules` is what CVE-2022-39253 abuses, so git
// refuses it unless the user allowed it; Cogit lifted that ban for every submodule.
#[test]
fn initializing_a_clone_keeps_git_s_ban_on_local_paths() {
    let f = test_fixtures::with_submodule().unwrap();
    let dir = fresh_clone(&f);
    let clone = RepoHandle::open(&dir.path().join("clone")).unwrap();

    assert!(transport_refused(
        clone.submodule_op(SubmoduleOp::Initialize, &[])
    ));
    assert!(transport_refused(clone.update_submodule(MODULE, true)));
    assert!(transport_refused(
        clone
            .init_submodules_added_since(&BTreeSet::new())
            .map(drop)
    ));
}
