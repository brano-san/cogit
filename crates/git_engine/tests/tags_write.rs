// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, TagRequest};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn names(repo: &RepoHandle) -> Vec<String> {
    repo.tags().unwrap().into_iter().map(|t| t.name).collect()
}

fn lightweight(name: &str) -> TagRequest {
    TagRequest {
        name: name.to_owned(),
        target: None,
        message: None,
        force: false,
    }
}

#[test]
fn a_lightweight_tag_points_at_head() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.create_tag(&lightweight("v1.0")).unwrap();

    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "v1.0")
        .unwrap();
    assert_eq!(tag.oid, f.oid("HEAD").unwrap());
    assert!(!tag.is_annotated);
}

#[test]
fn a_message_makes_the_tag_annotated() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.create_tag(&TagRequest {
        name: "v2.0".to_owned(),
        target: None,
        message: Some("second release".to_owned()),
        force: false,
    })
    .unwrap();

    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "v2.0")
        .unwrap();
    assert!(tag.is_annotated);
    assert_eq!(
        tag.oid,
        f.oid("HEAD").unwrap(),
        "an annotated tag still peels to the commit"
    );
}

#[test]
fn a_tag_can_point_at_an_older_commit() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let target = f.oid("HEAD~2").unwrap();

    repo.create_tag(&TagRequest {
        name: "start".to_owned(),
        target: Some(target.clone()),
        message: None,
        force: false,
    })
    .unwrap();

    let tag = repo
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.name == "start")
        .unwrap();
    assert_eq!(tag.oid, target);
}

#[test]
fn a_duplicate_name_is_refused_unless_forced() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    repo.create_tag(&lightweight("v1.0")).unwrap();

    assert!(repo.create_tag(&lightweight("v1.0")).is_err());

    repo.create_tag(&TagRequest {
        name: "v1.0".to_owned(),
        target: Some(f.oid("HEAD~1").unwrap()),
        message: None,
        force: true,
    })
    .unwrap();
    assert_eq!(
        repo.tags()
            .unwrap()
            .into_iter()
            .find(|t| t.name == "v1.0")
            .unwrap()
            .oid,
        f.oid("HEAD~1").unwrap()
    );
}

#[test]
fn deleting_a_tag_removes_it() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    repo.create_tag(&lightweight("temporary")).unwrap();

    repo.delete_tag("temporary").unwrap();

    assert!(!names(&repo).contains(&"temporary".to_owned()));
}

#[test]
fn deleting_a_tag_that_is_not_there_reports_gits_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let err = repo.delete_tag("never-existed").unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("never-existed"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn an_empty_name_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.create_tag(&lightweight("  ")).is_err());
    assert!(repo.delete_tag("").is_err());
}

#[test]
fn a_tag_name_with_slashes_works() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.create_tag(&lightweight("release/2026/09")).unwrap();

    assert!(names(&repo).contains(&"release/2026/09".to_owned()));
}

#[test]
fn tags_come_back_sorted() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    for name in ["v3", "v1", "v2"] {
        repo.create_tag(&lightweight(name)).unwrap();
    }

    let listed = names(&repo);
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
}

// The copy was made with git's default cleanup, which drops lines starting with `#` and
// trailing spaces: renaming `v1` took "#42 fixed" out of its notes.
#[test]
fn renaming_an_annotated_tag_keeps_its_message_to_the_letter() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&[
        "tag",
        "-a",
        "v1",
        "--cleanup=verbatim",
        "-m",
        "Release\n#42 fixed\n",
    ])
    .unwrap();
    let repo = git_engine::RepoHandle::open(f.path()).unwrap();

    repo.rename_tag("v1", "v2").unwrap();

    let message = repo.tag_message("v2").unwrap().unwrap();
    assert!(message.contains("#42 fixed"), "{message:?}");
}
