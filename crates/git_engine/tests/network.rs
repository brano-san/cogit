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

fn no_token(_url: &str) -> Option<String> {
    None
}

fn commands_of(repo: RepoHandle) -> (RepoHandle, Lines) {
    let log: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = repo.with_journal(Arc::new(move |out: git_engine::GitOutput| {
        if let Ok(mut entries) = sink.lock() {
            entries.push(out.command);
        }
    }));
    (repo, log)
}

/// What git makes of one `-c` argument for a URL, as a submodule's fetch would see it.
fn header_git_sends(arg: &str, url: &str) -> String {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&[
        "-c",
        arg,
        "config",
        "--get-urlmatch",
        "http.extraheader",
        url,
    ])
    .unwrap_or_default()
}

#[test]
fn fetch_brings_the_remote_tracking_branch_up_to_date() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.fetch("origin", no_token, on_line).unwrap();

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

    repo.fetch("origin", no_token, on_line).unwrap();

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

    let err = repo.fetch("nowhere", no_token, on_line).unwrap_err();

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

    repo.push("origin", None, false, no_token, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn a_rejected_push_reports_the_reason_in_full() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    let err = repo
        .push("origin", None, false, no_token, on_line)
        .unwrap_err();

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

// Push in the toolbar on a branch never pushed: with `push.default=simple` git refused,
// "The current branch feature has no upstream branch", on the first push of every branch.
#[test]
fn the_first_push_of_a_branch_publishes_it_and_sets_its_upstream() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "push.default", "simple"]).unwrap();
    f.git(&["config", "push.autoSetupRemote", "false"]).unwrap();
    f.git(&["switch", "-c", "feature"]).unwrap();
    f.commit_file(40, "feature.txt", "new\n").unwrap();
    let repo = open(&f);

    repo.push("origin", None, false, no_token, |_| {}).unwrap();

    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "feature@{upstream}"])
            .unwrap()
            .trim(),
        "origin/feature"
    );
    assert_eq!(
        f.oid("refs/remotes/origin/feature").unwrap(),
        f.oid("HEAD").unwrap()
    );
}

#[test]
fn a_branch_with_an_upstream_is_pushed_as_configured() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.commit_file(40, "to-push.txt", "pushed\n").unwrap();
    let (repo, log) = commands_of(open(&f));

    repo.push("origin", None, false, no_token, |_| {}).unwrap();

    let lines = seen(&log).join("\n");
    assert!(!lines.contains("--set-upstream"), "{lines}");
}

#[test]
fn a_forced_push_overwrites_the_remote() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = open(&f);
    let local = f.oid("HEAD").unwrap();
    let (_, on_line) = collector();

    repo.push("origin", None, true, no_token, on_line).unwrap();

    f.git(&["fetch", "origin"]).unwrap();
    assert_eq!(f.oid("refs/remotes/origin/main").unwrap(), local);
}

#[test]
fn pull_fast_forwards_when_nothing_is_local() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main~1"]).unwrap();
    let repo = open(&f);
    let (_, on_line) = collector();

    repo.pull("origin", true, no_token, on_line).unwrap();

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
    let before = f.oid("HEAD").unwrap();

    let err = repo.pull("origin", true, no_token, on_line).unwrap_err();

    // Without `--ff-only` a bare `git pull` fails too ("Need to specify how to reconcile"),
    // so only git's own reason proves which refusal this is.
    match err {
        git_engine::GitError::Command(details) => assert!(
            details.stderr.contains("Not possible to fast-forward"),
            "INV-05: {:?}",
            details.stderr
        ),
        other => panic!("expected git's refusal, got {other:?}"),
    }
    assert_eq!(
        f.oid("HEAD").unwrap(),
        before,
        "diverged history must not be merged behind the user's back"
    );
}

// F-311: Delete Merged Branches after Pull looks for branches whose upstream is gone, and a
// pull without `--prune` kept `origin/<branch>` after the server deleted it.
#[test]
fn a_pull_forgets_a_branch_the_remote_deleted() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    f.git(&["branch", "topic", "HEAD~1"]).unwrap();
    f.git(&["push", "--set-upstream", "origin", "topic"])
        .unwrap();
    let server = f.git(&["remote", "get-url", "origin"]).unwrap();
    f.git_in(
        std::path::Path::new(server.trim()),
        &["branch", "-D", "topic"],
    )
    .unwrap();
    let repo = open(&f);

    repo.pull("origin", true, no_token, |_| {}).unwrap();

    assert!(
        f.git(&["rev-parse", "--verify", "-q", "refs/remotes/origin/topic"])
            .is_err()
    );
    assert_eq!(repo.merged_gone_branches().unwrap(), ["topic"]);
}

// Preferences ▸ Pull ▸ Merge ran a bare `git pull`: with no `pull.rebase` git 2.33+ refuses
// diverged branches, and with `pull.rebase=true` (Git for Windows' installer default) it
// rebased instead.
#[test]
fn a_pull_in_merge_mode_merges_whatever_pull_rebase_says() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "pull.rebase", "true"]).unwrap();
    let local = f.oid("HEAD").unwrap();
    let repo = open(&f);

    repo.pull("origin", false, no_token, |_| {}).unwrap();

    let parents = f
        .git(&["rev-list", "--parents", "-n", "1", "HEAD"])
        .unwrap();
    let parents: Vec<&str> = parents.split_whitespace().skip(1).collect();
    assert_eq!(
        parents,
        [
            local.as_str(),
            f.oid("refs/remotes/origin/main").unwrap().as_str()
        ]
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
    f.git(&["remote", "set-url", "origin", "https://127.0.0.1:1/o/r.git"])
        .unwrap();
    let (repo, log) = commands_of(open(&f));

    let _ = repo.fetch("origin", |_| Some("s3cr3t".to_owned()), |_| {});

    let lines = seen(&log).join("\n");
    assert!(
        lines.contains("-c http.https://127.0.0.1:1/.extraHeader=<redacted> fetch"),
        "{lines}"
    );
    assert!(!lines.contains("s3cr3t"), "{lines}");
    assert!(
        !lines.contains(&git_engine::auth_header("s3cr3t")),
        "{lines}"
    );
}

// Git hands every `-c` to the git processes it starts (GIT_CONFIG_PARAMETERS), a
// submodule's fetch among them: a bare `http.extraHeader` sent the token to whatever host
// the submodule lives on.
#[test]
fn a_token_is_sent_only_to_the_host_of_the_remote() {
    let arg = git_engine::auth_config("https://github.com/o/r.git", "s3cr3t").unwrap();

    assert_eq!(
        header_git_sends(&arg, "https://github.com/o/r.git").trim(),
        git_engine::auth_header("s3cr3t")
    );
    assert_eq!(
        header_git_sends(&arg, "https://git.example.org/team/sub.git").trim(),
        ""
    );
}

#[test]
fn a_remote_that_is_not_http_gets_no_token() {
    assert!(git_engine::auth_config("git@github.com:o/r.git", "s3cr3t").is_none());
    assert!(git_engine::auth_config("/srv/git/r.git", "s3cr3t").is_none());
}

#[test]
fn a_host_scoped_header_is_hidden_in_the_journal_line() {
    let line = git_engine::redact_command(&[
        "-c",
        "http.https://github.com/.extraheader=Authorization: Basic c2VjcmV0",
        "fetch",
    ]);

    assert!(!line.contains("c2VjcmV0"), "{line}");
}

/// The URL each command asked a token for.
fn asked_for(run: impl FnOnce(&RepoHandle, &dyn Fn(&str) -> Option<String>)) -> Vec<String> {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&[
        "remote",
        "set-url",
        "origin",
        "https://127.0.0.1:1/fetch/r.git",
    ])
    .unwrap();
    f.git(&[
        "remote",
        "set-url",
        "--push",
        "origin",
        "https://127.0.0.1:2/push/r.git",
    ])
    .unwrap();
    let asked: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&asked);
    run(&open(&f), &move |url: &str| {
        sink.lock().unwrap().push(url.to_owned());
        None
    });
    seen(&asked)
}

// The token was looked up by the push URL for fetch and pull too, and the header with the
// push host's token went to the fetch host.
#[test]
fn fetch_and_pull_ask_for_the_token_of_the_fetch_url() {
    let fetched = asked_for(|repo, token| {
        let _ = repo.fetch("origin", |url| token(url), |_| {});
    });
    let pulled = asked_for(|repo, token| {
        let _ = repo.pull("origin", true, |url| token(url), |_| {});
    });

    assert_eq!(fetched, ["https://127.0.0.1:1/fetch/r.git"]);
    assert_eq!(pulled, ["https://127.0.0.1:1/fetch/r.git"]);
}

#[test]
fn push_asks_for_the_token_of_the_push_url() {
    let pushed = asked_for(|repo, token| {
        let _ = repo.push("origin", None, false, |url| token(url), |_| {});
    });

    assert_eq!(pushed, ["https://127.0.0.1:2/push/r.git"]);
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

    repo.fetch("origin", no_token, |_| {}).unwrap();

    assert!(!seen(&log).join("\n").contains("extraHeader"));
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
        .push("leaky", Some("master"), false, no_token, sink)
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

    let err = repo
        .push("origin", None, false, no_token, on_line)
        .unwrap_err();

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

/// A remote that takes the connection and never says a word: a fetch waits on it until
/// it is stopped.
fn silent_remote(f: &test_fixtures::Fixture) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let held: Vec<_> = server.incoming().take(4).collect();
        std::thread::sleep(std::time::Duration::from_secs(120));
        drop(held);
    });
    f.git(&[
        "remote",
        "add",
        "quiet",
        &format!("git://127.0.0.1:{port}/x.git"),
    ])
    .unwrap();
}

type Records = Arc<Mutex<Vec<git_engine::GitOutput>>>;

fn journalled(repo: RepoHandle) -> (RepoHandle, Records) {
    let log: Records = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = repo.with_journal(Arc::new(move |out: git_engine::GitOutput| {
        if let Ok(mut entries) = sink.lock() {
            entries.push(out);
        }
    }));
    (repo, log)
}

// The footer's cancel (DC-001): the git process goes, the command ends as cancelled and
// the journal says so, long before the silence watchdog would have stepped in.
#[test]
fn a_fetch_asked_to_stop_ends_as_cancelled() {
    let f = test_fixtures::linear(1).unwrap();
    silent_remote(&f);
    let stop = git_engine::NetworkStop::default();
    let (repo, log) = journalled(open(&f).with_stop(stop.clone()));
    let (done, finished) = std::sync::mpsc::channel();
    let started = std::time::Instant::now();

    std::thread::spawn(move || {
        let _ = done.send(repo.fetch("quiet", no_token, |_| {}));
    });
    while git_engine::children::running() == 0 {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "git never started"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(stop.stop(), "the first request is the one that stops it");
    let result = finished
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the fetch was still running 20 s after it was stopped");

    assert!(
        matches!(result, Err(git_engine::GitError::Cancelled(_))),
        "{result:?}"
    );
    assert!(!stop.stop(), "nothing is left to stop");
    let records = log.lock().unwrap();
    let last = records.last().expect("the stopped run is journalled");
    assert!(last.summary.contains("Cancelled"), "{last:?}");
}

#[test]
fn a_fetch_stopped_before_it_starts_never_runs_git() {
    let f = test_fixtures::linear(1).unwrap();
    silent_remote(&f);
    let stop = git_engine::NetworkStop::default();
    stop.stop();
    let (repo, log) = commands_of(open(&f).with_stop(stop));

    let result = repo.fetch("quiet", no_token, |_| {});

    assert!(
        matches!(result, Err(git_engine::GitError::Cancelled(_))),
        "{result:?}"
    );
    assert!(seen(&log).is_empty(), "{:?}", seen(&log));
}
