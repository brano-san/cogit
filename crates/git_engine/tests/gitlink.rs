// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A submodule is opened at its own path or not at all. Discovery walks upward, and from
//! an empty submodule directory it lands in the parent — which is how the tree once
//! listed a parent's submodules as a child's (doc/12-risks.md, R-149).

use git_engine::{GitError, ModuleProblem, RepoHandle};

fn problem(result: Result<RepoHandle, GitError>) -> ModuleProblem {
    match result {
        Err(GitError::ModuleUnavailable(problem)) => problem,
        Err(other) => panic!("expected a module problem, got {other:?}"),
        Ok(handle) => panic!("expected a failure, opened {}", handle.root().display()),
    }
}

#[test]
fn an_initialised_submodule_opens_at_its_own_root() {
    let f = test_fixtures::with_submodule().unwrap();
    let path = f.path().join("vendor/lib");
    let handle = RepoHandle::open_exact(&path).unwrap();
    assert_eq!(
        std::fs::canonicalize(handle.root()).unwrap(),
        std::fs::canonicalize(&path).unwrap()
    );
}

#[test]
fn a_nested_submodule_opens_at_its_own_root() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let path = f.path().join("vendor/middle/deep/inner");
    assert!(RepoHandle::open_exact(&path).is_ok());
}

#[test]
fn a_path_that_is_not_there_says_so() {
    let f = test_fixtures::with_submodule().unwrap();
    let path = f.path().join("vendor/nowhere");
    assert!(matches!(
        problem(RepoHandle::open_exact(&path)),
        ModuleProblem::Missing { .. }
    ));
}

#[test]
fn an_empty_submodule_directory_is_not_initialised_and_never_the_parent() {
    let f = test_fixtures::with_submodule().unwrap();
    let path = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    assert!(matches!(
        problem(RepoHandle::open_exact(&path)),
        ModuleProblem::NotInitialised { .. }
    ));
}

#[test]
fn a_git_file_pointing_nowhere_names_where_it_points() {
    let f = test_fixtures::with_submodule().unwrap();
    let path = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join(".git"), "gitdir: ../../.git/modules/gone\n").unwrap();
    match problem(RepoHandle::open_exact(&path)) {
        ModuleProblem::DanglingGitFile {
            target, foreign, ..
        } => {
            assert_eq!(target, "../../.git/modules/gone");
            assert!(!foreign, "a relative path belongs to no operating system");
        }
        other => panic!("expected a dangling .git file, got {other:?}"),
    }
}

/// The case from the second machine: a checkout made on Linux, read on Windows.
#[test]
fn a_git_file_with_another_systems_absolute_path_is_called_foreign() {
    let f = test_fixtures::with_submodule().unwrap();
    let path = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    let elsewhere = if cfg!(windows) {
        "/home/user/work/project/.git/modules/lib"
    } else {
        "C:/Users/user/work/project/.git/modules/lib"
    };
    std::fs::write(path.join(".git"), format!("gitdir: {elsewhere}\n")).unwrap();
    match problem(RepoHandle::open_exact(&path)) {
        ModuleProblem::DanglingGitFile {
            target, foreign, ..
        } => {
            assert_eq!(target, elsewhere);
            assert!(foreign);
        }
        other => panic!("expected a foreign .git file, got {other:?}"),
    }
}

#[test]
fn the_reason_is_worded_for_a_person() {
    let missing = ModuleProblem::Missing {
        path: "vendor/lib".to_owned(),
    };
    assert_eq!(missing.to_string(), "Directory does not exist: vendor/lib");

    let foreign = ModuleProblem::DanglingGitFile {
        path: "vendor/lib".to_owned(),
        target: "/home/user/x".to_owned(),
        foreign: true,
    };
    assert!(
        foreign
            .to_string()
            .contains("points to /home/user/x, which does not exist on this system"),
        "{foreign}"
    );
}
