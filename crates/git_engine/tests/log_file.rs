// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What reaches the log file at `info`, the level Preferences sets by default. The Output
//! window trims long output and promises the rest is in the log (03 §3).

use git_engine::RepoHandle;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

#[derive(Default, Clone)]
struct Capture(Arc<Mutex<Vec<String>>>);

struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl Subscriber for Capture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= Level::INFO
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(format!("{} ", event.metadata().level()));
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

fn logged_at_info(run: impl FnOnce()) -> String {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), run);
    capture.0.lock().unwrap().join("\n")
}

/// `git loud`: `lines` numbered lines on `stream` (1 or 2), then `exit`.
fn loud(lines: usize, stream: u8, exit: i32) -> String {
    format!(
        "alias.loud=!f() {{ i=0; while [ $i -lt {lines} ]; do echo line$i; i=$((i+1)); done >&{stream}; exit {exit}; }}; f"
    )
}

fn repo() -> (test_fixtures::Fixture, RepoHandle) {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    (f, repo)
}

// A pre-commit hook printed 30 000 lines and failed: the window showed the first 2 000
// and the last 5 000, and at `info` the log kept none of the rest.
#[test]
fn a_failure_too_long_for_the_window_is_in_the_log_whole() {
    let (_f, repo) = repo();
    let alias = loud(25_000, 2, 1);

    let log = logged_at_info(|| {
        let _ = repo.run_git(&["-c", &alias, "loud"]);
    });

    assert!(
        log.contains("line12000"),
        "the hidden middle is not in the log"
    );
}

#[test]
fn a_success_too_long_for_the_window_is_in_the_log_whole() {
    let (_f, repo) = repo();
    let alias = loud(25_000, 1, 0);

    let log = logged_at_info(|| {
        repo.run_git(&["-c", &alias, "loud"]).unwrap();
    });

    assert!(
        log.contains("line12000"),
        "the hidden middle is not in the log"
    );
}

#[test]
fn a_finished_command_is_logged_with_its_exit_code_and_time() {
    let (_f, repo) = repo();

    let log = logged_at_info(|| {
        repo.run_git(&["status", "--short"]).unwrap();
    });

    let finished = log
        .lines()
        .find(|line| line.contains("git status --short") && line.contains("exit_code"))
        .unwrap_or_else(|| panic!("no finish line:\n{log}"));
    assert!(finished.contains("exit_code=Some(0)"), "{finished}");
    assert!(finished.contains("duration_ms="), "{finished}");
}

#[test]
fn a_failed_fetch_is_logged_as_an_error_with_what_git_said() {
    let (_f, repo) = repo();

    let log = logged_at_info(|| {
        let _ = repo.fetch("nowhere", |_| None, |_| {});
    });

    let failed = log
        .lines()
        .find(|line| line.starts_with("ERROR") && line.contains("fetch"))
        .unwrap_or_else(|| panic!("no error line:\n{log}"));
    assert!(failed.contains("nowhere"), "{failed}");
}
