// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use git_engine::{GitOutput, PullOptions, RepoHandle};

type Log = Arc<Mutex<Vec<GitOutput>>>;

fn merge(ff_only: bool) -> PullOptions {
    PullOptions {
        ff_only,
        ..PullOptions::default()
    }
}

fn oid_at(repo: &RepoHandle, index: u32) -> String {
    repo.stashes().unwrap()[index as usize].oid.clone()
}

fn journaled(f: &test_fixtures::Fixture) -> (RepoHandle, Log) {
    let log: Log = Arc::default();
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path())
        .unwrap()
        .with_journal(Arc::new(move |out: GitOutput| {
            sink.lock().unwrap().push(out);
        }));
    (repo, log)
}

fn last(log: &Log) -> GitOutput {
    log.lock().unwrap().last().cloned().expect("a record")
}

fn two_sides() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(10, "file0.txt", "their line\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(11, "file0.txt", "our line\n").unwrap();
    f
}

#[test]
fn a_merge_that_stops_on_conflicts_is_marked() {
    let f = two_sides();
    let (repo, log) = journaled(&f);

    assert!(repo.run_git(&["merge", "theirs"]).is_err());

    let entry = last(&log);
    assert_eq!(entry.exit_code, Some(1));
    assert!(entry.stopped_on_conflicts);
}

#[test]
fn a_stash_apply_that_conflicts_is_marked() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "stashed\n").unwrap();
    f.git(&["stash", "push"]).unwrap();
    f.commit_file(12, "file0.txt", "committed\n").unwrap();
    let (repo, log) = journaled(&f);

    assert!(
        repo.stash_apply_index(&oid_at(&repo, 0), false, false)
            .is_err()
    );

    assert!(last(&log).stopped_on_conflicts);
}

#[test]
fn a_refusal_that_leaves_nothing_conflicted_is_not() {
    let f = two_sides();
    let (repo, log) = journaled(&f);

    assert!(repo.run_git(&["merge", "no-such-branch"]).is_err());

    assert!(!last(&log).stopped_on_conflicts);
}

#[test]
fn a_command_refused_because_conflicts_already_exist_is_not() {
    let f = two_sides();
    let (repo, log) = journaled(&f);
    let _ = repo.run_git(&["merge", "theirs"]);

    assert!(repo.run_git(&["merge", "theirs"]).is_err());
    assert!(!last(&log).stopped_on_conflicts);

    assert!(repo.run_git(&["commit", "-m", "too early"]).is_err());
    assert!(!last(&log).stopped_on_conflicts);
}

// `stash apply` refuses with exit 1, the same code as a stop, while an earlier apply's
// conflicts are unresolved: nothing was applied, yet the window said it stopped on conflicts.
#[test]
fn a_stash_apply_refused_over_earlier_conflicts_is_not() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "stashed
",
    )
    .unwrap();
    f.git(&["stash", "push"]).unwrap();
    std::fs::write(
        f.path().join("file1.txt"),
        "second
",
    )
    .unwrap();
    f.git(&["stash", "push"]).unwrap();
    f.commit_file(
        12,
        "file0.txt",
        "committed
",
    )
    .unwrap();
    let (repo, log) = journaled(&f);
    assert!(
        repo.stash_apply_index(&oid_at(&repo, 1), false, false)
            .is_err()
    );
    assert!(last(&log).stopped_on_conflicts);

    assert!(
        repo.stash_apply_index(&oid_at(&repo, 0), false, false)
            .is_err()
    );

    // Git 2.51 refuses with exit 1, the CI runner's with 128 ("could not write index"):
    // either way nothing was applied.
    let entry = last(&log);
    assert_ne!(entry.exit_code, Some(0), "{entry:?}");
    assert!(!entry.stopped_on_conflicts, "{entry:?}");
}

#[test]
fn a_command_that_succeeds_is_not() {
    let f = two_sides();
    let (repo, log) = journaled(&f);

    repo.run_git(&["status"]).unwrap();

    assert!(!last(&log).stopped_on_conflicts);
}

#[test]
fn a_cherry_pick_and_a_rebase_that_stop_are_marked() {
    let f = two_sides();
    let (repo, log) = journaled(&f);

    assert!(repo.run_git(&["cherry-pick", "theirs"]).is_err());
    assert!(last(&log).stopped_on_conflicts);

    repo.run_git(&["cherry-pick", "--abort"]).unwrap();
    assert!(repo.run_git(&["rebase", "theirs"]).is_err());
    assert!(last(&log).stopped_on_conflicts);
}

// A streamed run wrote to the journal itself, past the check, so a pull that stopped on
// conflicts was listed as a failed one.
#[test]
fn a_pull_that_stops_on_conflicts_is_marked() {
    let f = two_sides();
    let me = f.path().to_string_lossy().into_owned();
    f.git(&["remote", "add", "origin", &me]).unwrap();
    f.git(&["fetch", "origin"]).unwrap();
    f.git(&["branch", "--set-upstream-to=origin/theirs"])
        .unwrap();
    let (repo, log) = journaled(&f);

    assert!(
        repo.pull_options("origin", merge(false), |_| None, |_| {})
            .is_err()
    );

    let entry = last(&log);
    assert_eq!(entry.exit_code, Some(1), "{entry:?}");
    assert!(entry.stopped_on_conflicts);
}

#[test]
fn only_the_commands_that_stop_halfway_are_candidates() {
    use git_engine::outcome::stops_on_conflicts as stops;
    assert!(stops("git -c core.editor=true merge theirs", Some(1)));
    assert!(stops("git stash pop stash@{0}", Some(1)));
    assert!(!stops("git stash push", Some(1)));
    assert!(!stops("git merge theirs", Some(128)));
    assert!(!stops("git merge --abort", Some(1)));
    assert!(!stops("git commit -m x", Some(1)));
    assert!(stops(
        "git -c 'core.sshCommand=ssh -o BatchMode=yes' pull origin --no-rebase",
        Some(1)
    ));
    assert!(!stops(
        "git -c 'core.sshCommand=ssh -o BatchMode=yes' merge --abort",
        Some(1)
    ));
}

#[test]
fn a_refusal_turned_into_a_question_is_journaled_as_answered() {
    let f = test_fixtures::branched().unwrap();
    let (repo, log) = journaled(&f);

    repo.delete_branch("dev", false).unwrap();

    let entry = last(&log);
    assert_ne!(
        entry.exit_code,
        Some(0),
        "git's own words stay in the journal"
    );
    assert!(entry.answered, "the app asks instead: no error notice");
}

#[test]
fn a_plain_failure_is_not_answered() {
    let f = test_fixtures::linear(1).unwrap();
    let (repo, log) = journaled(&f);

    assert!(repo.delete_branch("missing", false).is_err());

    assert!(!last(&log).answered);
}
