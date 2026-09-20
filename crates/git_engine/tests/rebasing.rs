// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RebaseOptions, RepoHandle, RepoState};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn onto(target: &str) -> RebaseOptions {
    RebaseOptions {
        onto: target.to_owned(),
        autostash: false,
    }
}

fn count(f: &test_fixtures::Fixture) -> usize {
    f.git(&["rev-list", "--count", "HEAD"])
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn rebasing_replays_the_local_commits_on_top_of_the_target() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    let repo = open(&f);
    let before = count(&f);

    repo.rebase(&onto("main")).unwrap();

    assert_eq!(count(&f), before + 1, "main's own commit joins the history");
    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn the_rebased_commits_keep_their_messages() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    let repo = open(&f);

    repo.rebase(&onto("main")).unwrap();

    assert_eq!(repo.commit_details("HEAD").unwrap().summary, "commit 3");
}

#[test]
fn a_conflicting_rebase_stops_in_the_rebasing_state() {
    let f = test_fixtures::branched().unwrap();
    f.write_file("base.txt", "main version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(30, "change base on main").unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("base.txt", "dev version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(31, "change base on dev").unwrap();
    let repo = open(&f);

    let result = repo.rebase(&onto("main"));

    assert!(result.is_err());
    assert_eq!(repo.state().unwrap(), RepoState::Rebasing);
}

#[test]
fn an_interrupted_rebase_can_be_aborted_back_to_where_it_started() {
    let f = test_fixtures::branched().unwrap();
    f.write_file("base.txt", "main version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(30, "change base on main").unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("base.txt", "dev version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(31, "change base on dev").unwrap();
    let repo = open(&f);
    let before = f.oid("HEAD").unwrap();
    let _ = repo.rebase(&onto("main"));

    repo.abort_operation().unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn a_conflicting_rebase_can_be_skipped() {
    let f = test_fixtures::branched().unwrap();
    f.write_file("base.txt", "main version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(30, "change base on main").unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("base.txt", "dev version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(31, "change base on dev").unwrap();
    let repo = open(&f);
    let _ = repo.rebase(&onto("main"));

    repo.skip_operation().unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn autostash_carries_uncommitted_work_across_the_rebase() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("dev-1.txt", "uncommitted edit\n").unwrap();
    let repo = open(&f);

    repo.rebase(&RebaseOptions {
        onto: "main".to_owned(),
        autostash: true,
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("dev-1.txt")).unwrap(),
        "uncommitted edit\n",
        "the edit must survive the replay"
    );
}

#[test]
fn a_dirty_tree_without_autostash_is_refused_by_git() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("dev-1.txt", "uncommitted edit\n").unwrap();
    let repo = open(&f);

    assert!(repo.rebase(&onto("main")).is_err());
}

#[test]
fn rebasing_onto_something_unknown_reports_gits_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let err = repo.rebase(&onto("no-such-target")).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("no-such-target"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn an_empty_target_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).rebase(&onto("  ")).is_err());
}

#[test]
fn skipping_with_nothing_in_progress_is_refused() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).skip_operation().is_err());
}
