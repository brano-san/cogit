// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// A file whose lines come from three different commits.
fn layered() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("story.txt", "one\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    f.commit_staged(10, "first line").unwrap();

    f.write_file("story.txt", "one\ntwo\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    f.commit_staged(11, "second line").unwrap();

    f.write_file("story.txt", "one\ntwo\nthree\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    f.commit_staged(12, "third line").unwrap();
    f
}

#[test]
fn every_line_is_attributed() {
    let f = layered();

    let blame = open(&f).blame("story.txt", "HEAD").unwrap();

    assert_eq!(blame.len(), 3);
    assert!(blame.iter().all(|line| line.oid.len() == 40));
}

#[test]
fn a_line_is_attributed_to_the_commit_that_introduced_it() {
    let f = layered();
    let repo = open(&f);

    let blame = repo.blame("story.txt", "HEAD").unwrap();

    assert_eq!(blame[0].summary, "first line");
    assert_eq!(blame[1].summary, "second line");
    assert_eq!(blame[2].summary, "third line");
}

#[test]
fn each_line_carries_its_own_text_and_number() {
    let f = layered();

    let blame = open(&f).blame("story.txt", "HEAD").unwrap();

    assert_eq!(blame[1].line, 2);
    assert_eq!(blame[1].text, "two");
}

#[test]
fn the_author_and_date_come_along() {
    let f = layered();

    let blame = open(&f).blame("story.txt", "HEAD").unwrap();

    assert_eq!(blame[0].author, test_fixtures::AUTHOR_NAME);
    assert!(blame[0].timestamp > 0);
}

#[test]
fn an_untouched_line_keeps_its_original_commit_after_later_edits() {
    let f = layered();
    f.write_file("story.txt", "one\ntwo\nTHREE\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    f.commit_staged(13, "shout the third line").unwrap();
    let repo = open(&f);

    let blame = repo.blame("story.txt", "HEAD").unwrap();

    assert_eq!(blame[0].summary, "first line");
    assert_eq!(blame[2].summary, "shout the third line");
}

#[test]
fn blame_can_look_at_an_older_revision() {
    let f = layered();

    let blame = open(&f).blame("story.txt", "HEAD~1").unwrap();

    assert_eq!(blame.len(), 2, "the third line did not exist yet");
}

#[test]
fn a_path_that_is_not_in_the_revision_is_a_typed_error() {
    let f = layered();
    assert!(open(&f).blame("never-existed.txt", "HEAD").is_err());
}

#[test]
fn an_unknown_revision_is_a_typed_error() {
    let f = layered();
    assert!(open(&f).blame("story.txt", "no-such-rev").is_err());
}

#[test]
fn blame_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = layered();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.blame("story.txt", "HEAD").unwrap();

    assert!(log.lock().unwrap().is_empty(), "blame reads through gix");
}

#[test]
fn an_empty_file_blames_to_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("empty.txt", "").unwrap();
    f.git(&["add", "--", "empty.txt"]).unwrap();
    f.commit_staged(10, "add an empty file").unwrap();

    assert!(open(&f).blame("empty.txt", "HEAD").unwrap().is_empty());
}
