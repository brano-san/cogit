// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Preferences ▸ Git executable is process-wide, so it has a test binary of its own.

// Saved and never read: every command went on running the `git` on PATH (R-443).
#[test]
fn the_git_set_at_startup_is_the_one_every_command_runs() {
    let f = test_fixtures::linear(1).unwrap();
    let missing = f.path().join("no-such-git.exe");
    git_engine::use_git_program(missing.clone());

    let refused = git_engine::RepoHandle::open(f.path())
        .unwrap()
        .run_git(&["status"])
        .unwrap_err()
        .to_string();

    assert!(
        refused.contains(&missing.display().to_string()),
        "{refused}"
    );
    assert!(git_engine::git_version().is_err());
    assert!(matches!(
        git_engine::RepoHandle::open(f.path())
            .unwrap()
            .run_git(&["status"]),
        Err(git_engine::GitError::GitNotFound(_))
    ));

    // The streamed and the directly spawned commands name the missing git as well, so the
    // Fix dialog is offered for them (GR-08).
    let repo = git_engine::RepoHandle::open(f.path()).unwrap();
    assert!(matches!(
        repo.fetch("origin", |_| None, |_| {}),
        Err(git_engine::GitError::GitNotFound(_))
    ));
    assert!(matches!(
        repo.tag_name_problem("x"),
        Err(git_engine::GitError::GitNotFound(_))
    ));

    // Pointing to another git while the app runs takes effect for the next command.
    git_engine::use_git_program("git".into());
    assert!(git_engine::git_version().is_ok());
}
