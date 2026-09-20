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

#[test]
fn a_submodule_left_on_another_commit_is_reported_as_diverged() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = f.path().join("vendor/lib");
    let before = f.git_in(&inner, &["rev-parse", "HEAD"]).unwrap();
    f.git_in(&inner, &["checkout", "--detach", "HEAD~1"])
        .unwrap();

    let modules = open(&f).submodules().unwrap();

    assert_eq!(modules[0].state, SubmoduleState::Diverged);
    assert_eq!(modules[0].recorded, before.trim());
    assert_ne!(modules[0].checked_out.as_deref(), Some(before.trim()));
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
fn updating_a_diverged_submodule_puts_it_back_on_the_recorded_commit() {
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
