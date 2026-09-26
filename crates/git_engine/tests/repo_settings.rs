// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Repository ▸ Settings (#42): the repository's own values, and what applies without them.

use git_engine::{GitError, REPO_SETTING_KEYS, RepoHandle, RepoSetting, RepoSettingChange};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn setting(f: &test_fixtures::Fixture, key: &str) -> RepoSetting {
    open(f)
        .repo_settings()
        .into_iter()
        .find(|entry| entry.key == key)
        .unwrap_or_else(|| panic!("{key} is not offered"))
}

fn change(key: &str, value: Option<&str>) -> RepoSettingChange {
    RepoSettingChange {
        key: key.to_owned(),
        value: value.map(str::to_owned),
    }
}

fn local_config(f: &test_fixtures::Fixture, key: &str) -> Option<String> {
    f.git(&["config", "--local", "--get", key])
        .ok()
        .map(|value| value.trim().to_owned())
}

/// What git itself answers outside any repository: the user's and the system's config.
fn global_value(key: &str) -> Option<String> {
    let outside = tempfile::TempDir::new().unwrap();
    let output = test_fixtures::user_git_command(outside.path())
        .env("GIT_CEILING_DIRECTORIES", outside.path().parent().unwrap())
        .args(["config", "--get", key])
        .output()
        .unwrap();
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[test]
fn every_offered_key_is_answered_once() {
    let f = test_fixtures::linear(1).unwrap();
    let keys: Vec<String> = open(&f)
        .repo_settings()
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    assert_eq!(keys, REPO_SETTING_KEYS);
}

#[test]
fn a_value_in_the_repository_config_is_local() {
    let f = test_fixtures::linear(1).unwrap();
    assert_eq!(
        setting(&f, "user.name").local.as_deref(),
        Some(test_fixtures::AUTHOR_NAME)
    );
}

#[test]
fn a_key_the_repository_does_not_set_has_no_local_value() {
    let f = test_fixtures::linear(1).unwrap();
    assert_eq!(setting(&f, "push.default").local, None);
}

/// The inherited value is the one without the repository: the fixture sets `user.name`
/// locally, and that must not be reported as if it came from the user's own config.
#[test]
fn the_inherited_value_is_what_applies_without_the_repository() {
    let f = test_fixtures::linear(1).unwrap();
    assert_eq!(
        setting(&f, "user.name").inherited,
        global_value("user.name")
    );
}

#[test]
fn keys_are_matched_whatever_their_case() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "--local", "commit.gpgSign", "true"])
        .unwrap();
    assert_eq!(setting(&f, "commit.gpgSign").local.as_deref(), Some("true"));
}

#[test]
fn a_change_is_written_to_the_repository_config() {
    let f = test_fixtures::linear(1).unwrap();

    open(&f)
        .write_repo_settings(&[
            change("push.default", Some("simple")),
            change("cogit.tagGroupSeparator", Some("-")),
        ])
        .unwrap();

    assert_eq!(local_config(&f, "push.default").as_deref(), Some("simple"));
    assert_eq!(
        local_config(&f, "cogit.tagGroupSeparator").as_deref(),
        Some("-")
    );
    assert_eq!(setting(&f, "push.default").local.as_deref(), Some("simple"));
}

#[test]
fn no_value_removes_the_key_so_the_inherited_one_applies() {
    let f = test_fixtures::linear(1).unwrap();

    open(&f)
        .write_repo_settings(&[change("user.name", None)])
        .unwrap();

    assert_eq!(local_config(&f, "user.name"), None);
    assert_eq!(setting(&f, "user.name").local, None);
}

#[test]
fn removing_a_key_that_is_not_there_is_not_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    open(&f)
        .write_repo_settings(&[change("push.followTags", None)])
        .unwrap();
}

/// The dialog writes what it offers and nothing else: `core.hooksPath` is not a setting.
#[test]
fn a_key_the_dialog_does_not_offer_is_refused_and_nothing_is_written() {
    let f = test_fixtures::linear(1).unwrap();

    let refused = open(&f).write_repo_settings(&[
        change("push.default", Some("simple")),
        change("core.hooksPath", Some("elsewhere")),
    ]);

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "{refused:?}"
    );
    assert_eq!(local_config(&f, "push.default"), None);
    assert_eq!(local_config(&f, "core.hooksPath"), None);
}

#[test]
fn a_value_that_starts_with_a_dash_is_written_as_a_value() {
    let f = test_fixtures::linear(1).unwrap();
    open(&f)
        .write_repo_settings(&[change("cogit.tagGroupSeparator", Some("-"))])
        .unwrap();
    assert_eq!(
        setting(&f, "cogit.tagGroupSeparator").local.as_deref(),
        Some("-")
    );
}

/// Fetch and Pull ▸ Initialize new submodules is Cogit's own and read before a pull.
#[test]
fn init_new_submodules_is_off_unless_the_repository_says_so() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(!open(&f).wants_new_submodules());

    open(&f)
        .write_repo_settings(&[change("cogit.initNewSubmodules", Some("true"))])
        .unwrap();

    assert!(open(&f).wants_new_submodules());
}
