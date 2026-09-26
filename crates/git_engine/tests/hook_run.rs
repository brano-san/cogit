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
fn an_applypatch_msg_hook_is_given_a_message_and_reference_transaction_a_state() {
    let f = test_fixtures::linear(1).unwrap();
    let hooks = f.path().join(".git/hooks");
    write_hook(&hooks, "applypatch-msg", "#!/bin/sh\ncat \"$1\"\n");
    write_hook(&hooks, "reference-transaction", "#!/bin/sh\necho \"$1\"\n");

    let message = open(&f).run_hook("applypatch-msg").unwrap();
    let transaction = open(&f).run_hook("reference-transaction").unwrap();

    assert!(!message.stdout.trim().is_empty(), "{message:?}");
    assert_eq!(transaction.stdout.trim(), "prepared", "{transaction:?}");
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

// Cogit started from a terminal inside another repository's hook inherits its GIT_DIR.
// Git's own commands drop it (R-22); the hook run did not, and the hook's `git` then
// worked on that other repository.
#[test]
fn a_dry_run_does_not_hand_an_inherited_git_dir_to_the_hook() {
    let f = test_fixtures::linear(1).unwrap();
    let other = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\ngit rev-parse --absolute-git-dir\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_probe-hook"))
        .arg(f.path())
        .arg("pre-commit")
        .env("GIT_DIR", other.git_dir())
        .output()
        .unwrap();

    let printed = String::from_utf8_lossy(&out.stdout)
        .trim()
        .replace('\\', "/");
    let other_dir = other.git_dir().to_string_lossy().replace('\\', "/");
    assert!(out.status.success(), "{out:?}");
    assert!(
        !printed.eq_ignore_ascii_case(&other_dir),
        "the hook saw {printed}"
    );
}

// Started from the Start menu, Cogit has only Git\cmd from Git on PATH, and `bash` there
// is WSL's (System32 or WindowsApps\bash.exe): the dry run went into WSL or found nothing.
#[cfg(windows)]
#[test]
fn a_dry_run_uses_the_bash_of_git_for_windows_whatever_bash_path_finds() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "pre-commit",
        "#!/bin/sh\necho from-git-bash\n",
    );
    let exec_path = std::process::Command::new("git")
        .arg("--exec-path")
        .output()
        .unwrap();
    let exec_path = std::path::PathBuf::from(String::from_utf8_lossy(&exec_path.stdout).trim());
    let git_cmd = exec_path.ancestors().nth(3).unwrap().join("cmd");
    assert!(git_cmd.join("git.exe").is_file(), "{}", git_cmd.display());
    let impostor = tempfile::tempdir().unwrap();
    std::fs::copy(
        r"C:\Windows\System32\whoami.exe",
        impostor.path().join("bash.exe"),
    )
    .unwrap();
    let path = std::env::join_paths([impostor.path(), git_cmd.as_path()]).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_probe-hook"))
        .arg(f.path())
        .arg("pre-commit")
        .env("PATH", path)
        .output()
        .unwrap();

    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "from-git-bash");
}

// `bash <file>` reads the file as bash whatever its `#!` says: a Perl or Python hook that
// git runs fine failed its dry run with bash's syntax errors.
#[cfg(windows)]
#[test]
fn a_dry_run_honours_the_interpreter_the_hook_names() {
    let f = test_fixtures::linear(1).unwrap();
    write_hook(
        &f.path().join(".git/hooks"),
        "commit-msg",
        "#!/usr/bin/env perl\nprint \"ok\\n\";\n",
    );

    let run = open(&f).run_hook("commit-msg").unwrap();

    assert_eq!(run.exit_code, Some(0), "{run:?}");
    assert_eq!(run.stdout.trim(), "ok", "{run:?}");
}

/// A file in the user's home folder, removed again when the test ends.
struct InHome(std::path::PathBuf);

impl InHome {
    fn new(name: &str, text: &str) -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .expect("a home folder");
        let path = std::path::Path::new(&home).join(name);
        std::fs::write(&path, text).unwrap();
        Self(path)
    }
}

impl Drop for InHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// `commit.template = ~/.gitmessage` is the way it is usually written. Read as a plain
// string, `~` stayed a folder name under the repository and the template was not found.
#[test]
fn a_commit_template_under_the_home_folder_is_found() {
    let f = test_fixtures::linear(1).unwrap();
    let name = format!(".cogit-test-template-{}", std::process::id());
    let _file = InHome::new(&name, "Subject\n\nWhy:\n");
    f.git(&["config", "commit.template", &format!("~/{name}")])
        .unwrap();

    let template = open(&f).commit_template().unwrap();

    assert_eq!(template.as_deref(), Some("Subject\n\nWhy:\n"));
}
