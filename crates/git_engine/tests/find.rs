// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FoundKind, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn an_empty_query_finds_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(open(&f).find("  ", 20).unwrap().is_empty());
}

#[test]
fn a_branch_is_found_by_name() {
    let f = test_fixtures::branched().unwrap();

    let found = open(&f).find("dev", 20).unwrap();

    assert!(
        found
            .iter()
            .any(|item| item.kind == FoundKind::Branch && item.label == "dev"),
        "{found:?}"
    );
}

#[test]
fn a_tag_is_found_by_name() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "v1.0"]).unwrap();

    let found = open(&f).find("v1", 20).unwrap();

    assert!(
        found.iter().any(|item| item.kind == FoundKind::Tag),
        "{found:?}"
    );
}

#[test]
fn a_commit_is_found_by_its_short_hash() {
    let f = test_fixtures::linear(3).unwrap();
    let head = f.oid("HEAD").unwrap();

    let found = open(&f).find(&head[..7], 20).unwrap();

    assert!(
        found
            .iter()
            .any(|item| item.kind == FoundKind::Commit && item.oid == head),
        "{found:?}"
    );
}

#[test]
fn a_commit_is_found_by_its_full_hash() {
    let f = test_fixtures::linear(3).unwrap();
    let head = f.oid("HEAD").unwrap();

    let found = open(&f).find(&head, 20).unwrap();

    assert!(found.iter().any(|item| item.oid == head), "{found:?}");
}

#[test]
fn a_commit_is_found_by_words_from_its_message() {
    let f = test_fixtures::linear(4).unwrap();

    let found = open(&f).find("commit 2", 20).unwrap();

    assert!(
        found
            .iter()
            .any(|item| item.kind == FoundKind::Commit && item.label == "commit 2"),
        "{found:?}"
    );
}

#[test]
fn a_file_is_found_by_part_of_its_path() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/parser.rs"), "fn main() {}\n").unwrap();
    f.git(&["add", "--", "src/deep/parser.rs"]).unwrap();
    f.commit_staged(1, "add the parser").unwrap();

    let found = open(&f).find("parser.rs", 20).unwrap();

    assert!(
        found.iter().any(|item| item.kind == FoundKind::File),
        "{found:?}"
    );
}

#[test]
fn refs_are_listed_before_commits() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["tag", "dev-tag"]).unwrap();

    let found = open(&f).find("dev", 20).unwrap();
    let kinds: Vec<FoundKind> = found.iter().map(|item| item.kind).collect();

    let first_commit = kinds.iter().position(|k| *k == FoundKind::Commit);
    let last_ref = kinds
        .iter()
        .rposition(|k| matches!(k, FoundKind::Branch | FoundKind::Tag));
    if let (Some(commit), Some(reference)) = (first_commit, last_ref) {
        assert!(reference < commit, "refs come first: {kinds:?}");
    }
}

#[test]
fn the_limit_is_respected() {
    // More matches than the limit is the whole point; three spare is as good as fifteen.
    let f = test_fixtures::linear(8).unwrap();
    assert_eq!(open(&f).find("commit", 5).unwrap().len(), 5);
}

#[test]
fn a_query_matching_nothing_returns_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(open(&f).find("zzzz-nothing-zzzz", 20).unwrap().is_empty());
}

#[test]
fn finding_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::linear(5).unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.find("commit", 20).unwrap();

    assert!(log.lock().unwrap().is_empty());
}
