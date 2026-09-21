// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CommitRequest, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn request(message: &str, no_verify: bool) -> CommitRequest {
    CommitRequest {
        message: message.to_owned(),
        amend: false,
        no_verify,
        only: Vec::new(),
    }
}

fn stage_something(f: &test_fixtures::Fixture, name: &str) {
    std::fs::write(f.path().join(name), "content\n").unwrap();
    f.git(&["add", "--", name]).unwrap();
}

#[test]
fn a_fresh_repository_has_bypassed_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).bypass_log().unwrap().is_empty());
}

#[test]
fn committing_with_no_verify_is_recorded() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    open(&f).commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(open(&f).bypass_log().unwrap().len(), 1);
}

#[test]
fn an_ordinary_commit_is_not_recorded_as_a_bypass() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    open(&f).commit(&request("run the hooks", false)).unwrap();

    assert!(open(&f).bypass_log().unwrap().is_empty());
}

#[test]
fn the_record_names_the_commit_it_belongs_to() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);

    let oid = repo.commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(repo.bypass_log().unwrap()[0].oid, oid);
}

#[test]
fn the_record_keeps_the_subject_so_the_summary_reads_sensibly() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);

    repo.commit(&request("skip the hooks", true)).unwrap();

    assert_eq!(repo.bypass_log().unwrap()[0].summary, "skip the hooks");
}

#[test]
fn several_bypasses_accumulate_newest_first() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    for name in ["a.txt", "b.txt"] {
        stage_something(&f, name);
        repo.commit(&request(&format!("bypass for {name}"), true))
            .unwrap();
    }

    let log = repo.bypass_log().unwrap();

    assert_eq!(log.len(), 2);
    assert_eq!(log[0].summary, "bypass for b.txt", "{log:?}");
}

#[test]
fn a_corrupt_record_is_skipped_rather_than_failing_the_read() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");
    let repo = open(&f);
    repo.commit(&request("skip the hooks", true)).unwrap();

    let path = f.path().join(".git/cogit-hook-bypasses");
    let good = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("nonsense without separators\n{good}")).unwrap();

    assert_eq!(repo.bypass_log().unwrap().len(), 1);
}

#[test]
fn a_commit_that_fails_is_not_recorded_as_a_bypass() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.commit(&request("nothing is staged", true)).is_err());
    assert!(repo.bypass_log().unwrap().is_empty());
}

#[test]
fn a_repository_without_a_template_offers_none() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).commit_template().unwrap().is_none());
}

#[test]
fn a_configured_template_is_read() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join(".gitmessage"), "type(scope): \n\nWhy:\n").unwrap();
    f.git(&["config", "commit.template", ".gitmessage"])
        .unwrap();

    assert_eq!(
        open(&f).commit_template().unwrap().as_deref(),
        Some("type(scope): \n\nWhy:\n")
    );
}

#[test]
fn a_template_pointing_at_a_missing_file_is_not_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "commit.template", "nowhere.txt"])
        .unwrap();

    assert!(open(&f).commit_template().unwrap().is_none());
}

#[test]
fn a_template_path_is_resolved_against_the_repository_root() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("etc")).unwrap();
    std::fs::write(f.path().join("etc/msg.txt"), "from etc\n").unwrap();
    f.git(&["config", "commit.template", "etc/msg.txt"])
        .unwrap();

    assert_eq!(
        open(&f).commit_template().unwrap().as_deref(),
        Some("from etc\n")
    );
}

#[test]
fn a_bypass_reaches_the_output_panel_as_its_own_line() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = std::sync::Arc::clone(&log);
        std::sync::Arc::new(move |entry: git_engine::GitOutput| {
            log.lock().unwrap().push(entry);
        })
    };

    open(&f)
        .with_journal(sink)
        .commit(&request("skip the hooks", true))
        .unwrap();

    let entries = log.lock().unwrap();
    let bypass = entries
        .iter()
        .find(|entry| entry.command.contains("hooks bypassed"))
        .expect("the bypass is journalled");
    // Exit zero with something on stderr is what the panel marks as a warning.
    assert_eq!(bypass.exit_code, Some(0));
    assert!(!bypass.stderr.trim().is_empty(), "{bypass:?}");
}

#[test]
fn an_ordinary_commit_writes_no_bypass_line() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = std::sync::Arc::clone(&log);
        std::sync::Arc::new(move |entry: git_engine::GitOutput| {
            log.lock().unwrap().push(entry);
        })
    };

    open(&f)
        .with_journal(sink)
        .commit(&request("run the hooks", false))
        .unwrap();

    let entries = log.lock().unwrap();
    assert!(!entries.iter().any(|e| e.command.contains("hooks bypassed")));
}

#[test]
fn the_bypass_line_names_the_commit_it_belongs_to() {
    let f = test_fixtures::linear(1).unwrap();
    stage_something(&f, "a.txt");

    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = std::sync::Arc::clone(&log);
        std::sync::Arc::new(move |entry: git_engine::GitOutput| {
            log.lock().unwrap().push(entry);
        })
    };

    let oid = open(&f)
        .with_journal(sink)
        .commit(&request("skip the hooks", true))
        .unwrap();

    let entries = log.lock().unwrap();
    let bypass = entries
        .iter()
        .find(|entry| entry.command.contains("hooks bypassed"))
        .unwrap();
    assert!(bypass.stderr.contains(&oid[..7]), "{bypass:?}");
}
