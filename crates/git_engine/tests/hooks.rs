// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{HookSource, HookState, RepoHandle};
use std::path::Path;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn write_hook(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
}

#[test]
fn every_git_hook_is_listed_even_when_the_repository_has_none() {
    let f = test_fixtures::linear(1).unwrap();

    let overview = open(&f).hooks().unwrap();

    assert!(overview.hooks.len() >= 10, "{}", overview.hooks.len());
    assert!(
        overview
            .hooks
            .iter()
            .all(|hook| hook.state == HookState::Missing)
    );
}

#[test]
fn every_hook_carries_a_description_of_when_it_runs() {
    let f = test_fixtures::linear(1).unwrap();

    assert!(
        open(&f)
            .hooks()
            .unwrap()
            .hooks
            .iter()
            .all(|hook| !hook.description.is_empty())
    );
}

#[test]
fn a_hook_in_the_git_directory_is_found() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 0\n",
    );

    let overview = open(&f).hooks().unwrap();
    let hook = overview
        .hooks
        .iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();

    assert_eq!(hook.state, HookState::Enabled);
    assert_eq!(hook.source, Some(HookSource::GitHooks));
}

#[test]
fn the_sample_hooks_git_installs_are_not_counted_as_present() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit.sample",
        "#!/bin/sh\n",
    );

    let overview = open(&f).hooks().unwrap();
    let hook = overview
        .hooks
        .iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();

    assert_eq!(hook.state, HookState::Missing);
}

#[test]
fn a_configured_hooks_path_wins_over_the_git_directory() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".git/hooks"), "pre-commit", "#!/bin/sh\n");
    write_hook(&f.path().join(".githooks"), "pre-commit", "#!/bin/sh\n");
    f.git(&["config", "core.hooksPath", ".githooks"]).unwrap();

    let overview = open(&f).hooks().unwrap();
    let hook = overview
        .hooks
        .iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();

    assert_eq!(hook.source, Some(HookSource::HooksPath));
    assert_eq!(overview.configured_path.as_deref(), Some(".githooks"));
}

#[test]
fn an_unconfigured_hooks_directory_in_the_tree_is_reported_as_available() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".githooks"), "pre-commit", "#!/bin/sh\n");

    let overview = open(&f).hooks().unwrap();

    assert!(overview.configured_path.is_none());
    assert_eq!(overview.available_path.as_deref(), Some(".githooks"));
}

#[test]
fn a_disabled_hook_is_listed_as_disabled_rather_than_missing() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit.disabled",
        "#!/bin/sh\n",
    );

    let overview = open(&f).hooks().unwrap();
    let hook = overview
        .hooks
        .iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();

    assert_eq!(hook.state, HookState::Disabled);
}

#[test]
fn disabling_a_hook_keeps_its_body() {
    let f = test_fixtures::linear(1).unwrap();
    let body = "#!/bin/sh\necho hello\n";
    write_hook(&f.path().join(".git/hooks"), "pre-commit", body);
    let repo = open(&f);

    repo.set_hook_enabled("pre-commit", false).unwrap();

    assert_eq!(repo.read_hook("pre-commit").unwrap(), body);
    assert!(!f.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn enabling_a_disabled_hook_brings_it_back() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit.disabled",
        "#!/bin/sh\n",
    );
    let repo = open(&f);

    repo.set_hook_enabled("pre-commit", true).unwrap();

    assert!(f.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn writing_a_hook_normalises_crlf_so_the_shebang_works() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.write_hook("pre-commit", "#!/bin/sh\r\nexit 0\r\n")
        .unwrap();

    let written = std::fs::read(f.path().join(".git/hooks/pre-commit")).unwrap();
    assert!(!written.contains(&b'\r'), "{written:?}");
}

#[test]
fn a_written_hook_is_executable() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.write_hook("pre-commit", "#!/bin/sh\nexit 0\n")
        .unwrap();

    let hook = repo
        .hooks()
        .unwrap()
        .hooks
        .into_iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();
    assert!(hook.executable);
}

#[test]
fn reading_a_hook_that_does_not_exist_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).read_hook("pre-commit").is_err());
}

#[test]
fn a_name_that_is_not_a_git_hook_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.write_hook("../../evil.sh", "#!/bin/sh\n").is_err());
    assert!(repo.read_hook("not-a-hook").is_err());
}

#[test]
fn listing_hooks_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::linear(1).unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.hooks().unwrap();

    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn a_repository_without_gitattributes_needs_the_eol_rule() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".githooks"), "pre-commit", "#!/bin/sh\n");

    assert!(open(&f).needs_eol_rule(".githooks").unwrap());
}

#[test]
fn a_repository_that_already_pins_lf_for_the_hooks_needs_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".githooks"), "pre-commit", "#!/bin/sh\n");
    f.write_file(".gitattributes", ".githooks/** eol=lf\n")
        .unwrap();

    assert!(!open(&f).needs_eol_rule(".githooks").unwrap());
}

#[test]
fn a_rule_for_another_directory_does_not_count() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".githooks"), "pre-commit", "#!/bin/sh\n");
    f.write_file(".gitattributes", "scripts/** eol=lf\n")
        .unwrap();

    assert!(open(&f).needs_eol_rule(".githooks").unwrap());
}

#[test]
fn the_eol_rule_can_be_added_without_losing_what_is_there() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.png binary\n").unwrap();

    open(&f).add_eol_rule(".githooks").unwrap();

    let text = std::fs::read_to_string(f.path().join(".gitattributes")).unwrap();
    assert!(text.contains("*.png binary"), "{text}");
    assert!(text.contains(".githooks/** eol=lf"), "{text}");
}

// A comment saved in cp1251 made the file unreadable as UTF-8, and the rule was written
// over it: every other attribute the team had was gone.
#[test]
fn a_gitattributes_that_is_not_utf8_keeps_its_bytes() {
    let f = test_fixtures::linear(1).unwrap();
    let mut original = b"# \xcf\xf0\xe8\xec\xe5\xf0\n*.png binary\n".to_vec();
    std::fs::write(f.path().join(".gitattributes"), &original).unwrap();

    open(&f).add_eol_rule(".githooks").unwrap();

    original.extend_from_slice(b".githooks/** eol=lf\n");
    assert_eq!(
        std::fs::read(f.path().join(".gitattributes")).unwrap(),
        original
    );
}

#[test]
fn adding_the_rule_twice_does_not_repeat_it() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.add_eol_rule(".githooks").unwrap();
    repo.add_eol_rule(".githooks").unwrap();

    let text = std::fs::read_to_string(f.path().join(".gitattributes")).unwrap();
    assert_eq!(text.matches(".githooks/** eol=lf").count(), 1, "{text}");
}

#[test]
fn every_hook_in_the_active_directory_is_checked_for_the_execution_bit() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(&f.path().join(".git/hooks"), "pre-commit", "#!/bin/sh\n");

    let overview = open(&f).hooks().unwrap();
    let present: Vec<_> = overview
        .hooks
        .iter()
        .filter(|hook| hook.state != HookState::Missing)
        .collect();

    assert_eq!(present.len(), 1);
}

// A linked worktree has a private git directory, but git runs the hooks from the common
// one. The panel read `<private>/hooks`, which does not exist, and showed every hook
// missing while git kept running them.
#[test]
fn a_linked_worktree_lists_the_hooks_git_runs_there() {
    let f = test_fixtures::linear(1).unwrap();
    let aux = tempfile::tempdir().unwrap();
    let linked = aux.path().join("linked");
    f.git(&[
        "worktree",
        "add",
        "-b",
        "wt",
        &linked.to_string_lossy().replace('\\', "/"),
    ])
    .unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 0\n",
    );

    let overview = RepoHandle::open(&linked).unwrap().hooks().unwrap();
    let hook = overview
        .hooks
        .iter()
        .find(|hook| hook.name == "pre-commit")
        .unwrap();

    assert_eq!(hook.state, HookState::Enabled);
}
