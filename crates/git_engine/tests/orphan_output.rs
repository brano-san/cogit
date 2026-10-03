#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A command ends when git exits, not when every copy of its pipes is closed: a hook's
//! background process holds them for as long as it lives (GR-04).

use git_engine::RepoHandle;
use std::time::{Duration, Instant};

fn hook_that_leaves_a_process_behind(f: &test_fixtures::Fixture, name: &str) {
    let dir = f.path().join(".git/hooks");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nsleep 20 &\necho hook-done >&2\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn a_hook_that_leaves_a_process_behind_does_not_hold_the_command() {
    let f = test_fixtures::linear(1).unwrap();
    hook_that_leaves_a_process_behind(&f, "post-commit");
    let repo = RepoHandle::open(f.path()).unwrap();
    let started = Instant::now();

    let out = repo
        .run_git(&["commit", "--allow-empty", "-m", "x"])
        .unwrap();

    assert!(
        started.elapsed() < test_fixtures::scaled(Duration::from_secs(10)),
        "{:?}",
        started.elapsed()
    );
    assert!(out.stderr.contains("hook-done"), "{out:?}");
}

#[test]
fn a_hook_that_leaves_a_process_behind_does_not_hold_a_push() {
    let f = test_fixtures::with_remote().unwrap();
    hook_that_leaves_a_process_behind(&f, "pre-push");
    let repo = RepoHandle::open(f.path()).unwrap();
    f.git(&["commit", "--allow-empty", "-m", "x"]).unwrap();
    let started = Instant::now();

    let mut lines = Vec::new();
    repo.push(
        "origin",
        Some("HEAD:refs/heads/topic"),
        false,
        |_| None,
        |line| lines.push(line.to_owned()),
    )
    .unwrap();

    assert!(
        started.elapsed() < test_fixtures::scaled(Duration::from_secs(10)),
        "{:?}",
        started.elapsed()
    );
    assert!(
        lines.iter().any(|line| line.contains("hook-done")),
        "{lines:?}"
    );
}
