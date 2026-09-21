// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use std::sync::{Arc, Mutex};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

type Lines = Arc<Mutex<Vec<String>>>;

fn collector() -> (Lines, impl FnMut(&str)) {
    let lines: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    (lines, move |line: &str| {
        if let Ok(mut entries) = sink.lock() {
            entries.push(line.to_owned());
        }
    })
}

fn seen(lines: &Lines) -> Vec<String> {
    lines.lock().map(|l| l.clone()).unwrap_or_default()
}

#[test]
fn fetch_brings_the_remote_tracking_branch_up_to_date() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.fetch("origin", None, on_line).unwrap();

    assert!(
        repo.branches()
            .unwrap()
            .iter()
            .any(|b| b.name == "origin/main"),
        "the tracking branch must come back"
    );
}

#[test]
fn fetch_reports_what_git_says_while_it_runs() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    let repo = open(&f);
    let (lines, on_line) = collector();

    repo.fetch("origin", None, on_line).unwrap();

    assert!(
        seen(&lines).iter().any(|l| !l.trim().is_empty()),
        "a fetch that moves refs always says something"
    );
}

#[test]
fn fetching_an_unknown_remote_reports_gits_own_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.fetch("nowhere", None, on_line).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("nowhere"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn push_sends_local_commits_to_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.commit_file(40, "to-push.txt", "pushed\n").unwrap();
    let repo = open(&f);
    let local = f.oid("HEAD").unwrap();
    let (_, on_line) = collector();

    repo.push("origin", None, false, None, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn a_rejected_push_reports_the_reason_in_full() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.push("origin", None, false, None, on_line).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(
                details.stderr.contains("reject") || details.stderr.contains("fetch first"),
                "INV-05: the user needs Git's exact reason, got {:?}",
                details.stderr
            );
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn a_forced_push_overwrites_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let local = f.oid("HEAD").unwrap();
    let (_, on_line) = collector();

    repo.push("origin", None, true, None, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn pull_fast_forwards_when_nothing_is_local() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main~1"]).unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.pull("origin", true, None, on_line).unwrap();

    assert_eq!(
        f.oid("HEAD").unwrap(),
        f.oid("refs/remotes/origin/main").unwrap()
    );
}

#[test]
fn a_pull_that_cannot_fast_forward_is_refused_rather_than_merging_silently() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.pull("origin", true, None, on_line);

    assert!(
        err.is_err(),
        "diverged history must not be merged behind the user's back"
    );
}

#[test]
fn the_remote_list_is_read_without_spawning_a_process() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    let remotes = repo.remotes().unwrap();

    assert_eq!(remotes, vec!["origin".to_owned()]);
    assert!(seen(&log).is_empty());
}

#[test]
fn a_token_reaches_git_as_a_header_but_not_the_journal() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.fetch("origin", Some("s3cr3t"), |_| {}).unwrap();

    let lines = seen(&log).join("\n");
    assert!(lines.contains("http.extraHeader="), "{lines}");
    assert!(!lines.contains("s3cr3t"), "{lines}");
    assert!(
        !lines.contains(&git_engine::auth_header("s3cr3t")),
        "{lines}"
    );
}

#[test]
fn no_token_means_no_extra_argument() {
    let f = test_fixtures::with_remote().unwrap();
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.fetch("origin", None, |_| {}).unwrap();

    assert!(!seen(&log).join("\n").contains("http.extraHeader"));
}

/// End to end, with a real `git push` against a URL that carries a token: nothing the
/// user could paste into a bug report carries the credential.
///
/// Modern git already hides it in its own messages; this pins that, and `output_text`
/// covers the paths git does not control — hook output, and older versions.
#[test]
fn a_token_in_a_remote_url_never_leaves_the_runner() {
    const TOKEN: &str = "ghp_thisMustNotAppearAnywhere";

    let f = test_fixtures::linear(1).unwrap();
    f.git(&[
        "remote",
        "add",
        "leaky",
        &format!("https://brano:{TOKEN}@127.0.0.1:1/x/y.git"),
    ])
    .unwrap();

    let repo = open(&f);
    let (lines, sink) = collector();
    let err = repo
        .push("leaky", Some("master"), false, None, sink)
        .expect_err("pushing at a dead port must fail");

    let git_engine::GitError::Command(details) = err else {
        panic!("expected a command failure");
    };

    let everywhere = format!(
        "{} {} {} {} {}",
        details.command,
        details.stdout,
        details.stderr,
        details.summary,
        seen(&lines).join(" ")
    );
    assert!(
        !everywhere.contains(TOKEN),
        "the token survived: {everywhere}"
    );
    assert_eq!(
        details.operation, "Push",
        "the heading names the command that ran"
    );
    assert!(!details.summary.is_empty(), "a failure must say something");
}

/// The scenario the window exists for: `pre-push` runs the test suite, one test panics,
/// and every line of that panic has to survive the trip to the reader.
#[test]
fn a_failing_pre_push_hook_delivers_its_whole_log() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.commit_file(41, "work.txt", "done\n").unwrap();

    let hooks = f.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-push");
    std::fs::write(
        &hook,
        "#!/bin/sh\n\
         echo 'running 2 tests' >&2\n\
         echo 'test budget::a_commit_is_under_a_second ... FAILED' >&2\n\
         echo '' >&2\n\
         echo \"thread 'budget' panicked at crates/app_state/tests/mutation_speed.rs:31:5:\" >&2\n\
         echo '  assertion failed: elapsed < BUDGET' >&2\n\
         echo 'note: run with `RUST_BACKTRACE=1` to display a backtrace' >&2\n\
         exit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut mode = std::fs::metadata(&hook).unwrap().permissions();
        mode.set_mode(0o755);
        std::fs::set_permissions(&hook, mode).unwrap();
    }

    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo.push("origin", None, false, None, on_line).unwrap_err();

    let git_engine::GitError::Command(details) = err else {
        panic!("a rejected push must be a command failure");
    };
    for line in [
        "test budget::a_commit_is_under_a_second ... FAILED",
        "thread 'budget' panicked at crates/app_state/tests/mutation_speed.rs:31:5:",
        "  assertion failed: elapsed < BUDGET",
        "note: run with `RUST_BACKTRACE=1` to display a backtrace",
    ] {
        assert!(
            details.stderr.contains(line),
            "the hook said {line:?} and the reader never saw it:\n{}",
            details.stderr
        );
    }
    assert!(
        details.stderr.contains("FAILED\n\nthread"),
        "the blank line between them carries the shape of the log:\n{}",
        details.stderr
    );
    assert_eq!(details.operation, "Push");
}
