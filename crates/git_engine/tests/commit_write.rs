// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CommitRequest, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn request(message: &str) -> CommitRequest {
    CommitRequest {
        message: message.to_owned(),
        amend: false,
        no_verify: false,
        only: Vec::new(),
    }
}

#[test]
fn commits_what_is_staged_and_returns_the_new_oid() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    let oid = repo.commit(&request("add fresh.txt")).unwrap();

    assert_eq!(oid, f.oid("HEAD").unwrap());
    assert!(repo.worktree_files().unwrap().staged.is_empty());
    assert_eq!(repo.commit_details(&oid).unwrap().summary, "add fresh.txt");
}

#[test]
fn a_multi_line_message_keeps_its_body() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    let oid = repo
        .commit(&request("subject line\n\nbody one\nbody two"))
        .unwrap();

    let details = repo.commit_details(&oid).unwrap();
    assert_eq!(details.summary, "subject line");
    assert_eq!(details.body, "body one\nbody two");
}

#[test]
fn the_first_commit_of_an_empty_repository_works() {
    let f = test_fixtures::empty().unwrap();
    std::fs::write(f.path().join("first.txt"), "content\n").unwrap();
    f.git(&["add", "--", "first.txt"]).unwrap();
    let repo = open(&f);

    let oid = repo.commit(&request("first commit")).unwrap();

    assert!(repo.commit_details(&oid).unwrap().parents.is_empty());
}

#[test]
fn nothing_staged_is_refused_with_gits_own_message() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let err = repo.commit(&request("nothing to say")).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(
                details.stdout.contains("nothing") || details.stderr.contains("nothing"),
                "INV-05: Git explains this better than we would, got {details:?}"
            );
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn an_empty_message_is_refused_before_git_is_even_started() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    assert!(repo.commit(&request("   ")).is_err());
    assert_eq!(
        repo.worktree_files().unwrap().staged.len(),
        1,
        "a refused commit must leave the index alone"
    );
}

#[test]
fn amend_replaces_the_previous_commit_instead_of_adding_one() {
    let f = test_fixtures::linear(3).unwrap();
    let before = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    let repo = open(&f);

    let oid = repo
        .commit(&CommitRequest {
            message: "commit 2, reworded".to_owned(),
            amend: true,
            no_verify: false,
            only: Vec::new(),
        })
        .unwrap();

    let after = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(before.trim(), after.trim());
    assert_eq!(
        repo.commit_details(&oid).unwrap().summary,
        "commit 2, reworded"
    );
}

#[test]
fn amend_can_add_staged_changes_to_the_previous_commit() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("extra.txt"), "late\n").unwrap();
    f.git(&["add", "--", "extra.txt"]).unwrap();
    let repo = open(&f);

    let oid = repo
        .commit(&CommitRequest {
            message: "commit 0".to_owned(),
            amend: true,
            no_verify: false,
            only: Vec::new(),
        })
        .unwrap();

    let paths: Vec<String> = repo
        .commit_files(&oid)
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert!(paths.contains(&"extra.txt".to_owned()), "{paths:?}");
}

#[test]
fn a_message_that_looks_like_a_flag_is_not_parsed_as_one() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    let oid = repo.commit(&request("--amend is not a flag here")).unwrap();

    assert_eq!(
        repo.commit_details(&oid).unwrap().summary,
        "--amend is not a flag here"
    );
    assert_eq!(repo.commit_details(&oid).unwrap().parents.len(), 1);
}

#[test]
fn no_verify_skips_a_hook_that_would_reject_the_commit() {
    let f = test_fixtures::linear(1).unwrap();
    let hooks = f.git_dir().join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(
        hooks.join("pre-commit"),
        "#!/bin/sh\necho refused by the hook >&2\nexit 1\n",
    )
    .unwrap();

    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    assert!(repo.commit(&request("blocked")).is_err());

    let oid = repo
        .commit(&CommitRequest {
            message: "allowed".to_owned(),
            amend: false,
            no_verify: true,
            only: Vec::new(),
        })
        .unwrap();
    assert_eq!(repo.commit_details(&oid).unwrap().summary, "allowed");
}

#[test]
fn committing_only_named_paths_leaves_the_rest_staged() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["one.txt", "two.txt"] {
        std::fs::write(f.path().join(name), "new\n").unwrap();
    }
    f.git(&["add", "--", "one.txt", "two.txt"]).unwrap();
    let repo = open(&f);

    repo.commit(&CommitRequest {
        message: "only one".to_owned(),
        amend: false,
        no_verify: false,
        only: vec!["one.txt".to_owned()],
    })
    .unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(
        files
            .staged
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        ["two.txt"],
        "the hidden file must stay staged"
    );
}

#[test]
fn an_empty_path_list_still_commits_everything_staged() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["one.txt", "two.txt"] {
        std::fs::write(f.path().join(name), "new\n").unwrap();
    }
    f.git(&["add", "--", "one.txt", "two.txt"]).unwrap();
    let repo = open(&f);

    repo.commit(&request("both of them")).unwrap();

    assert!(repo.worktree_files().unwrap().staged.is_empty());
}

// `rev-parse HEAD` after the commit was a second process, 40 ms on Windows (R-313).
#[test]
fn committing_starts_one_process_and_still_returns_the_new_oid() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = std::sync::Arc::clone(&commands);
    let repo = open(&f).with_journal(std::sync::Arc::new(move |out: git_engine::GitOutput| {
        sink.lock().unwrap().push(out.command);
    }));

    let oid = repo.commit(&request("add fresh.txt")).unwrap();

    assert_eq!(oid, f.oid("HEAD").unwrap());
    let commands = commands.lock().unwrap();
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert!(commands[0].contains(" commit "), "{commands:?}");
}

#[test]
fn amending_returns_the_oid_of_the_amended_commit() {
    let f = test_fixtures::linear(2).unwrap();
    let before = f.oid("HEAD").unwrap();
    let repo = open(&f);

    let oid = repo
        .commit(&CommitRequest {
            amend: true,
            ..request("reworded")
        })
        .unwrap();

    assert_ne!(oid, before);
    assert_eq!(oid, f.oid("HEAD").unwrap());
}
