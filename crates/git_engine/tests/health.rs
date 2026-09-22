// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What SmartGit warns about when a repository is opened, and what the second machine
//! needed to be told: a clone made on Linux and used from Windows (doc/12-risks.md, R-150).

use git_engine::{HealthIssue, RepoHandle, case_sensitive};

/// The fixtures write a fixed `.git/config` without `core.ignorecase`, which on Windows is
/// exactly what a Linux clone looks like. `git init` itself writes the value it probed.
fn as_git_writes_it(f: &test_fixtures::Fixture, repo: &std::path::Path) {
    let insensitive = !case_sensitive(repo).unwrap();
    f.git_in(
        repo,
        &[
            "config",
            "core.ignorecase",
            if insensitive { "true" } else { "false" },
        ],
    )
    .unwrap();
}

fn issues(handle: &RepoHandle) -> Vec<HealthIssue> {
    handle
        .health_report()
        .into_iter()
        .filter(|finding| finding.module.is_empty())
        .map(|finding| finding.issue)
        .collect()
}

#[test]
fn the_file_system_is_asked_rather_than_guessed_from_the_platform() {
    let dir = tempfile::tempdir().unwrap();
    let sensitive = case_sensitive(dir.path()).unwrap();
    if cfg!(windows) {
        assert!(!sensitive, "NTFS folders are case-insensitive by default");
    } else if cfg!(target_os = "linux") {
        assert!(sensitive);
    }
}

#[test]
fn the_probe_leaves_nothing_behind() {
    let f = test_fixtures::linear(1).unwrap();
    let before: Vec<_> = std::fs::read_dir(f.git_dir()).unwrap().collect();
    let _ = RepoHandle::open(f.path()).unwrap().health_report();
    let after: Vec<_> = std::fs::read_dir(f.git_dir()).unwrap().collect();
    assert_eq!(before.len(), after.len());
}

#[test]
fn a_repository_git_made_here_is_healthy() {
    let f = test_fixtures::linear(2).unwrap();
    as_git_writes_it(&f, f.path());
    assert!(issues(&RepoHandle::open(f.path()).unwrap()).is_empty());
}

#[test]
fn ignore_case_that_disagrees_with_the_file_system_is_reported() {
    let f = test_fixtures::linear(1).unwrap();
    let actual_insensitive = !case_sensitive(&f.git_dir()).unwrap();
    let wrong = if actual_insensitive { "false" } else { "true" };
    f.git(&["config", "core.ignorecase", wrong]).unwrap();

    let found = issues(&RepoHandle::open(f.path()).unwrap());
    assert_eq!(
        found,
        vec![HealthIssue::IgnoreCaseMismatch {
            configured: !actual_insensitive,
            actual: actual_insensitive,
        }]
    );
}

/// Git writes `core.ignorecase = true` on a case-insensitive file system and nothing at
/// all on Linux; a missing key read on Windows is the fingerprint of a Linux clone.
#[cfg(windows)]
#[test]
fn a_missing_ignore_case_on_windows_counts_as_false() {
    let f = test_fixtures::linear(1).unwrap();
    let found = issues(&RepoHandle::open(f.path()).unwrap());
    assert!(
        found
            .iter()
            .any(|issue| matches!(issue, HealthIssue::IgnoreCaseMismatch { .. })),
        "{found:?}"
    );
}

#[test]
fn a_worktree_whose_folder_is_gone_is_reported() {
    let f = test_fixtures::with_worktree().unwrap();
    as_git_writes_it(&f, f.path());
    let linked = worktree_path(&f);
    std::fs::remove_dir_all(&linked).unwrap();

    let found = issues(&RepoHandle::open(f.path()).unwrap());
    match found.as_slice() {
        [HealthIssue::DanglingWorktree { foreign, .. }] => assert!(!foreign),
        other => panic!("expected one dangling worktree, got {other:?}"),
    }
}

#[test]
fn a_worktree_written_by_another_system_is_called_foreign() {
    let f = test_fixtures::with_worktree().unwrap();
    as_git_writes_it(&f, f.path());
    let elsewhere = if cfg!(windows) {
        "/home/user/work/linked/.git"
    } else {
        "C:/Users/user/work/linked/.git"
    };
    let gitdir = only_worktree_admin(&f).join("gitdir");
    std::fs::write(&gitdir, format!("{elsewhere}\n")).unwrap();

    let found = issues(&RepoHandle::open(f.path()).unwrap());
    match found.as_slice() {
        [
            HealthIssue::DanglingWorktree {
                target, foreign, ..
            },
        ] => {
            assert_eq!(target, elsewhere);
            assert!(foreign);
        }
        other => panic!("expected one foreign worktree, got {other:?}"),
    }
}

#[test]
fn a_submodule_whose_git_file_points_nowhere_is_reported_under_its_path() {
    let f = test_fixtures::with_submodule().unwrap();
    let module = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&module).unwrap();
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(module.join(".git"), "gitdir: /home/user/gone\n").unwrap();

    let report = RepoHandle::open(f.path()).unwrap().health_report();
    let finding = report
        .iter()
        .find(|finding| finding.module == "vendor/lib")
        .expect("the submodule is named");
    assert!(matches!(finding.issue, HealthIssue::DanglingModule { .. }));
}

#[test]
fn a_nested_submodule_is_checked_as_well_as_the_top() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let inner = f.path().join("vendor/middle/deep/inner");
    let insensitive = !case_sensitive(&f.git_dir()).unwrap();
    let wrong = if insensitive { "false" } else { "true" };
    f.git_in(&inner, &["config", "core.ignorecase", wrong])
        .unwrap();

    let report = RepoHandle::open(f.path()).unwrap().health_report();
    assert!(
        report
            .iter()
            .any(|finding| finding.module == "vendor/middle/deep/inner"
                && matches!(finding.issue, HealthIssue::IgnoreCaseMismatch { .. })),
        "{report:?}"
    );
}

#[test]
fn an_uninitialised_submodule_is_not_mistaken_for_a_broken_one() {
    let f = test_fixtures::with_submodule().unwrap();
    let module = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&module).unwrap();
    std::fs::create_dir_all(&module).unwrap();

    let report = RepoHandle::open(f.path()).unwrap().health_report();
    assert!(
        report.iter().all(|finding| finding.module != "vendor/lib"),
        "{report:?}"
    );
}

fn only_worktree_admin(f: &test_fixtures::Fixture) -> std::path::PathBuf {
    let admin = f.git_dir().join("worktrees");
    let mut entries: Vec<_> = std::fs::read_dir(&admin)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1);
    entries.remove(0)
}

fn worktree_path(f: &test_fixtures::Fixture) -> std::path::PathBuf {
    let gitdir = std::fs::read_to_string(only_worktree_admin(f).join("gitdir")).unwrap();
    std::path::Path::new(gitdir.trim())
        .parent()
        .unwrap()
        .to_path_buf()
}
