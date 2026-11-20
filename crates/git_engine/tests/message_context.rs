#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{Omitted, RepoHandle};

#[test]
fn the_context_holds_the_staged_diff_the_branch_and_the_subjects() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["checkout", "-q", "-b", "feature/ABC-1"]).unwrap();
    f.write_file("src.txt", "one\ntwo\n").unwrap();
    f.write_file("Cargo.lock", "[[package]]\nname = \"x\"\n")
        .unwrap();
    f.write_file("secret.bin", "text marked -diff\n").unwrap();
    f.write_file(".gitattributes", "secret.bin -diff\n")
        .unwrap();
    f.write_file("unstaged.txt", "not staged\n").unwrap();
    f.git(&[
        "add",
        "src.txt",
        "Cargo.lock",
        "secret.bin",
        ".gitattributes",
    ])
    .unwrap();
    f.write_file(".gitattributes", "secret.bin diff\n").unwrap();

    let context = RepoHandle::open(f.path())
        .unwrap()
        .message_context(2)
        .unwrap();

    assert_eq!(context.branch.as_deref(), Some("feature/ABC-1"));
    assert_eq!(context.subjects.len(), 2);
    assert!(context.diff.contains("diff --git a/src.txt b/src.txt"));
    assert!(context.diff.contains("+two"));
    assert!(!context.diff.contains("unstaged"));
    assert!(!context.diff.contains("[[package]]"));
    assert!(!context.diff.contains("text marked"));
    let reason = |path: &str| {
        context
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap()
            .omitted
    };
    assert_eq!(reason("src.txt"), None);
    assert_eq!(reason("Cargo.lock"), Some(Omitted::Lockfile));
    assert_eq!(reason("secret.bin"), Some(Omitted::Binary));
}

#[test]
fn an_unborn_branch_has_no_subjects() {
    let f = test_fixtures::empty().unwrap();
    f.write_file("first.txt", "hello\n").unwrap();
    f.git(&["add", "first.txt"]).unwrap();

    let context = RepoHandle::open(f.path())
        .unwrap()
        .message_context(5)
        .unwrap();

    assert!(context.subjects.is_empty());
    assert!(context.diff.contains("+hello"));
}
