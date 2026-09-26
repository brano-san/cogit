// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Checking out with the changes carried over (F-132, item 46 of 25.09): stash, check out
//! and apply are one call, so the lane runs them as one operation (CC-008 of the audit). The
//! stash is kept out of the list while it runs and joins it only when it has to stay.

use git_engine::{AutostashOutcome, CheckoutTarget, Head, RepoHandle, StashOptions};

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

/// What the autostash left in Cogit's own namespace: nothing, once it is done.
fn backups(f: &test_fixtures::Fixture) -> String {
    f.git(&["for-each-ref", "--format=%(refname)", "refs/cogit/backup/"])
        .unwrap()
}

const MESSAGE: &str = "cogit: autostash";

#[test]
fn the_switch_goes_through_and_the_changes_come_back() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(
        repo.checkout(&topic()).is_err(),
        "the fixture must be in the way"
    );

    let outcome = repo.switch_with_autostash(&topic(), MESSAGE, true).unwrap();

    assert_eq!(outcome, AutostashOutcome::Restored);
    assert_eq!(head_branch(&repo), "topic");
    assert_eq!(
        read(&f, "a.txt"),
        BASE.replacen('1', "one", 1).replace("10\n", "ten\n")
    );
    assert!(repo.stashes().unwrap().is_empty());
    assert_eq!(backups(&f), "");
}

// The checkbox "drop the stash once it applies cleanly", unticked.
#[test]
fn the_stash_stays_listed_when_asked_to_keep_it() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();

    let outcome = repo
        .switch_with_autostash(&topic(), MESSAGE, false)
        .unwrap();

    assert_eq!(outcome, AutostashOutcome::Kept { clean: true });
    assert_eq!(
        read(&f, "a.txt"),
        BASE.replacen('1', "one", 1).replace("10\n", "ten\n")
    );
    let listed = repo.stashes().unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].message.contains(MESSAGE), "{listed:?}");
    assert_eq!(backups(&f), "");
}

#[test]
fn a_refused_switch_puts_the_changes_back_and_says_why() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    let missing = CheckoutTarget::Branch {
        name: "no-such-branch".to_owned(),
    };

    let refused = repo.switch_with_autostash(&missing, MESSAGE, true);

    assert!(refused.is_err());
    assert_eq!(head_branch(&repo), "main");
    assert_eq!(read(&f, "a.txt"), BASE.replace("10\n", "ten\n"));
    assert!(repo.stashes().unwrap().is_empty());
    assert_eq!(backups(&f), "");
}

// They went back unstaged: the put-back after a refusal was a `pop` without `--index`.
#[test]
fn a_refused_switch_puts_staged_changes_back_staged() {
    let f = in_the_way();
    f.git(&["add", "a.txt"]).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let missing = CheckoutTarget::Branch {
        name: "no-such-branch".to_owned(),
    };

    assert!(repo.switch_with_autostash(&missing, MESSAGE, true).is_err());

    let staged: Vec<String> = repo
        .worktree_files()
        .unwrap()
        .staged
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    assert_eq!(staged, ["a.txt"]);
}

#[test]
fn a_stash_that_fails_leaves_head_where_it_was() {
    let f = in_the_way();
    let repo = RepoHandle::open(f.path()).unwrap();
    std::fs::write(f.git_dir().join("index.lock"), "").unwrap();

    assert!(repo.switch_with_autostash(&topic(), MESSAGE, true).is_err());

    assert_eq!(head_branch(&repo), "main");
    assert_eq!(read(&f, "a.txt"), BASE.replace("10\n", "ten\n"));
}

// Item 46: with conflicts the stash stays, and in the list, where the user can find it. It
// used to be an error of the whole call, though the checkout had gone through.
#[test]
fn changes_that_conflict_after_the_switch_stay_listed() {
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

    let outcome = repo.switch_with_autostash(&topic(), MESSAGE, true).unwrap();

    assert_eq!(outcome, AutostashOutcome::Kept { clean: false });
    assert_eq!(head_branch(&repo), "topic");
    let listed = repo.stashes().unwrap();
    assert_eq!(listed.len(), 1, "the stash the changes could not leave");
    assert!(listed[0].message.contains(MESSAGE), "{listed:?}");
    assert_eq!(backups(&f), "");
}

// R-521: another stash operation shifted `stash@{0}` between the steps, and the changes came
// back from someone else's stash. The autostash is never in the list now, so the user's
// stashes are left exactly as they were.
#[test]
fn the_users_own_stashes_stay_as_they_were() {
    let f = in_the_way();
    let options = StashOptions {
        message: "mine".to_owned(),
        include_untracked: true,
        keep_index: false,
    };
    let repo = RepoHandle::open(f.path()).unwrap();
    repo.stash_push_if_any(&options).unwrap().unwrap();
    f.write_file("a.txt", &BASE.replace("10\n", "ten\n"))
        .unwrap();
    let before = repo.stashes().unwrap();

    repo.switch_with_autostash(&topic(), MESSAGE, true).unwrap();

    assert_eq!(repo.stashes().unwrap(), before);
    assert_eq!(
        read(&f, "a.txt"),
        BASE.replacen('1', "one", 1).replace("10\n", "ten\n")
    );
}
