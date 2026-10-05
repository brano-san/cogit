//! The diff view's editor: what it reads, what a save writes, and when it refuses (F-722).
// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::editable::{EditableFile, SaveOutcome};
use git_engine::{DiffSpec, RepoHandle};

fn read(repo: &RepoHandle, spec: &DiffSpec) -> EditableFile {
    repo.read_editable(spec, "file0.txt").unwrap()
}

#[test]
fn a_save_keeps_crlf_and_the_left_side_is_the_index() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "one\r\ntwo\r\n").unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let EditableFile::Text {
        base,
        text,
        shape,
        stamp,
    } = read(&repo, &DiffSpec::WorkTreeVsIndex)
    else {
        panic!("refused");
    };
    assert_eq!(text, "one\ntwo");
    assert!(!base.is_empty() && !base.contains('\r'), "{base:?}");

    let saved = repo
        .save_editable("file0.txt", "one\nnew\ntwo", shape, &stamp, false)
        .unwrap();
    assert!(matches!(saved, SaveOutcome::Saved { .. }));
    assert_eq!(
        std::fs::read(f.path().join("file0.txt")).unwrap(),
        b"one\r\nnew\r\ntwo\r\n"
    );
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten_unless_forced() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let EditableFile::Text { shape, stamp, .. } = read(&repo, &DiffSpec::WorkTreeVsIndex) else {
        panic!("refused");
    };
    std::fs::write(f.path().join("file0.txt"), "theirs\n").unwrap();

    let saved = repo
        .save_editable("file0.txt", "mine", shape, &stamp, false)
        .unwrap();
    assert!(matches!(saved, SaveOutcome::ChangedOnDisk));
    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "theirs\n"
    );

    repo.save_editable("file0.txt", "mine", shape, &stamp, true)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "mine\n"
    );
}

#[test]
fn a_staged_or_committed_side_is_never_editable() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    assert!(matches!(
        read(&repo, &DiffSpec::IndexVsHead),
        EditableFile::Refused { .. }
    ));
    let head = repo.commit_details("HEAD").unwrap().oid;
    let commits = DiffSpec::CommitVsCommit {
        a: head.clone(),
        b: head,
    };
    assert!(matches!(
        read(&repo, &commits),
        EditableFile::Refused { .. }
    ));
}
