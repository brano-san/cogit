// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Switching with the changes stashed out of the way (F-132): stash, switch and put back
//! are one call, so the lane runs them as one operation (CC-008 of the audit).

use git_engine::{CheckoutTarget, Head, RepoHandle, StashOptions};

const BASE: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";

/// `a.txt` differs on `topic` in its first line; the working tree of `main` changes its last
/// one, so `git switch topic` refuses and the changes apply cleanly once there.
fn in_the_way() -> test_fixtures::Fixture {
    let f = test_fixtures::Fixture::init().unwrap();
    f.commit_file(0, "a.txt", BASE).unwrap();
    f.git(&["switch", "--create", "topic"]).unwrap();
    f.commit_file(1, "a.txt", &BASE.replacen('1', "one", 1))
        .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.write_file("a.txt", &BASE.replace("10\n", "ten\n"))
        .unwrap();
    f
}

fn topic() -> CheckoutTarget {
    CheckoutTarget::Branch {
        name: "topic".to_owned(),
    }
}

fn head_branch(repo: &RepoHandle) -> String {
    match repo.head().unwrap() {
        Head::Branch { name, .. } => name,
        other => panic!("expected an attached HEAD, got {other:?}"),
    }
}

fn read(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

#[test]
fn the_switch_goes_through_and_the_changes_come_back() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(
        repo.checkout(&topic()).is_err(),
        "the fixture must be in the way"
    );

    repo.switch_with_autostash(&topic(), "cogit: autostash")
        .unwrap();

    assert_eq!(head_branch(&repo), "topic");
    assert_eq!(
        read(&f, "a.txt"),
        BASE.replacen('1', "one", 1).replace("10\n", "ten\n")
    );
    assert!(repo.stashes().unwrap().is_empty());
}

#[test]
fn a_refused_switch_puts_the_changes_back_and_says_why() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    let missing = CheckoutTarget::Branch {
        name: "no-such-branch".to_owned(),
    };

    let refused = repo.switch_with_autostash(&missing, "cogit: autostash");

    assert!(refused.is_err());
    assert_eq!(head_branch(&repo), "main");
    assert_eq!(read(&f, "a.txt"), BASE.replace("10\n", "ten\n"));
    assert!(repo.stashes().unwrap().is_empty());
}

#[test]
fn a_stash_that_fails_leaves_head_where_it_was() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    std::fs::write(f.git_dir().join("index.lock"), "").unwrap();

    assert!(
        repo.switch_with_autostash(&topic(), "cogit: autostash")
            .is_err()
    );

    assert_eq!(head_branch(&repo), "main");
    assert_eq!(read(&f, "a.txt"), BASE.replace("10\n", "ten\n"));
}

#[test]
fn a_pop_that_conflicts_after_the_switch_keeps_the_stash() {
    let f = in_the_way();
    // The last line differs on topic too, so the changes cannot go back cleanly.
    f.git(&["stash"]).unwrap();
    f.git(&["switch", "topic"]).unwrap();
    f.commit_file(
        2,
        "a.txt",
        &BASE.replacen('1', "one", 1).replace("10\n", "TEN\n"),
    )
    .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["stash", "pop"]).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    assert!(
        repo.switch_with_autostash(&topic(), "cogit: autostash")
            .is_err()
    );

    assert_eq!(head_branch(&repo), "topic");
    assert_eq!(
        repo.stashes().unwrap().len(),
        1,
        "git keeps a stash it could not apply"
    );
}

// Another stash operation queued between the steps shifted `stash@{0}`: the changes were
// then taken back from someone else's stash, and it was dropped.
#[test]
fn the_changes_come_back_from_the_stash_made_for_them() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    f.write_file("file0.txt", "mine\n").unwrap();
    let options = |message: &str| StashOptions {
        message: message.to_owned(),
        include_untracked: true,
        keep_index: false,
    };
    let made = repo
        .stash_push_if_any(&options("autostash"))
        .unwrap()
        .unwrap();
    f.write_file("file1.txt", "someone else's\n").unwrap();
    repo.stash_push_if_any(&options("other")).unwrap().unwrap();

    repo.stash_pop_oid(&made).unwrap();

    assert_eq!(read(&f, "file0.txt"), "mine\n");
    let left = repo.stashes().unwrap();
    assert_eq!(left.len(), 1);
    assert!(left[0].message.contains("other"), "{left:?}");
}
