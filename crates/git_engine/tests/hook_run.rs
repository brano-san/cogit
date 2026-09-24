// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use std::path::Path;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn write_hook(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join(name);
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
}

#[test]
fn a_hook_that_succeeds_reports_a_zero_exit_code() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 0\n",
    );

    let run = open(&f).run_hook("pre-commit").unwrap();

    assert_eq!(run.exit_code, Some(0));
}

#[test]
fn the_output_of_a_hook_is_returned_in_full() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\necho to-stdout\necho to-stderr >&2\n",
    );

    let run = open(&f).run_hook("pre-commit").unwrap();

    assert!(run.stdout.contains("to-stdout"), "{run:?}");
    assert!(run.stderr.contains("to-stderr"), "{run:?}");
}

#[test]
fn a_failing_hook_is_a_result_not_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 3\n",
    );

    let run = open(&f).run_hook("pre-commit").unwrap();

    assert_eq!(run.exit_code, Some(3));
}

#[test]
fn a_dry_run_leaves_the_repository_untouched() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 0\n",
    );
    let before = f.oid("HEAD").unwrap();

    open(&f).run_hook("pre-commit").unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn a_commit_msg_hook_is_given_a_draft_message_to_read() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "commit-msg",
        "#!/bin/sh\ncat \"$1\"\n",
    );

    let run = open(&f).run_hook("commit-msg").unwrap();

    assert!(!run.stdout.trim().is_empty(), "{run:?}");
}

#[test]
fn the_run_is_timed_so_a_slow_hook_can_be_spotted() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\nexit 0\n",
    );

    let run = open(&f).run_hook("pre-commit").unwrap();

    assert!(run.duration_ms < 60_000, "{run:?}");
}

#[test]
fn running_a_hook_that_is_not_there_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).run_hook("pre-commit").is_err());
}

#[test]
fn a_disabled_hook_is_not_run() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit.disabled",
        "#!/bin/sh\nexit 0\n",
    );

    assert!(open(&f).run_hook("pre-commit").is_err());
}

// Two dry runs of commit-msg shared one message file: the first to finish deleted it
// under the second, whose hook then failed on a message that was never missing.
#[test]
fn two_dry_runs_at_once_keep_their_own_message() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "commit-msg",
        "#!/bin/sh\ntest -f \"$1\" || exit 3\nsleep 1\ntest -f \"$1\" || exit 4\n",
    );

    let first = std::thread::spawn({
        let root = f.path().to_path_buf();
        move || {
            RepoHandle::open(&root)
                .unwrap()
                .run_hook("commit-msg")
                .unwrap()
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(500));
    let second = open(&f).run_hook("commit-msg").unwrap();
    let first = first.join().unwrap();

    assert_eq!(first.exit_code, Some(0), "{}", first.stderr);
    assert_eq!(second.exit_code, Some(0), "{}", second.stderr);
}
