// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{BisectMark, BisectState, GitError, RepoHandle, RepoState};

/// Commit `i` of `linear` adds `file{i}.txt`; the bug arrives with `file{BUG}.txt`.
const COMMITS: i64 = 16;
const BUG: i64 = 10;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn commit(f: &test_fixtures::Fixture, index: i64) -> String {
    f.oid(&format!("main~{}", COMMITS - 1 - index)).unwrap()
}

fn bisect(repo: &RepoHandle) -> BisectState {
    match repo.state().unwrap() {
        RepoState::Bisecting { bisect } => bisect,
        other => panic!("expected a bisect, got {other:?}"),
    }
}

fn has_bug(f: &test_fixtures::Fixture) -> bool {
    f.path().join(format!("file{BUG}.txt")).exists()
}

#[test]
fn starting_with_a_bad_and_a_good_commit_checks_out_one_between() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    let repo = open(&f);

    repo.bisect_start("HEAD", Some(&commit(&f, 0))).unwrap();

    let state = bisect(&repo);
    assert_eq!(state.start, "main");
    assert_eq!(state.bad.as_deref(), Some(commit(&f, COMMITS - 1).as_str()));
    assert_eq!(state.good, [commit(&f, 0)]);
    let current = state.current.expect("a commit is checked out to test");
    let between: Vec<String> = (1..COMMITS - 1).map(|i| commit(&f, i)).collect();
    assert!(
        between.contains(&current),
        "{current} is not between the two"
    );
    assert_eq!(state.first_bad, None);
}

#[test]
fn marking_each_tested_commit_ends_at_the_first_bad_one() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    let repo = open(&f);
    repo.bisect_start("HEAD", Some(&commit(&f, 0))).unwrap();

    let mut steps = 0;
    while bisect(&repo).first_bad.is_none() {
        let mark = if has_bug(&f) {
            BisectMark::Bad
        } else {
            BisectMark::Good
        };
        repo.bisect_mark(mark, None).unwrap();
        steps += 1;
        assert!(steps <= 5, "log2(16) + 1 steps are enough");
    }

    let state = bisect(&repo);
    assert_eq!(state.first_bad.as_deref(), Some(commit(&f, BUG).as_str()));
    assert!(state.candidates.is_empty());
}

#[test]
fn a_commit_other_than_head_can_be_marked() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    let repo = open(&f);
    repo.bisect_start("HEAD", None).unwrap();
    assert!(bisect(&repo).good.is_empty(), "no good commit yet");

    repo.bisect_mark(BisectMark::Good, Some(&commit(&f, 3)))
        .unwrap();

    let state = bisect(&repo);
    assert_eq!(state.good, [commit(&f, 3)]);
    assert!(state.current.is_some());
}

#[test]
fn a_skipped_commit_is_remembered_and_another_is_checked_out() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    let repo = open(&f);
    repo.bisect_start("HEAD", Some(&commit(&f, 0))).unwrap();
    let tested = bisect(&repo).current.unwrap();

    repo.bisect_mark(BisectMark::Skip, None).unwrap();

    let state = bisect(&repo);
    assert_eq!(state.skipped, std::slice::from_ref(&tested));
    assert_ne!(state.current.as_deref(), Some(tested.as_str()));
}

#[test]
fn only_skipped_commits_left_names_the_candidates() {
    let f = test_fixtures::linear(4).unwrap();
    let repo = open(&f);
    let oid = |rev: &str| f.oid(rev).unwrap();
    repo.bisect_start("main", Some(&oid("main~2"))).unwrap();
    assert_eq!(bisect(&repo).current, Some(oid("main~1")));

    // git exits 2 here; its words stay the error, and the state says what is left.
    let refused = repo.bisect_mark(BisectMark::Skip, None);
    assert!(
        matches!(refused, Err(GitError::Command(_))),
        "got {refused:?}"
    );

    let state = bisect(&repo);
    assert_eq!(state.first_bad, None);
    assert!(
        state.candidates.contains(&oid("main~1")),
        "got {:?}",
        state.candidates
    );
}

#[test]
fn reset_goes_back_to_the_branch_the_bisect_began_on() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    let repo = open(&f);
    let tip = commit(&f, COMMITS - 1);
    repo.bisect_start("HEAD", Some(&commit(&f, 0))).unwrap();

    repo.bisect_reset().unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
    assert_eq!(
        repo.head().unwrap(),
        git_engine::Head::Branch {
            name: "main".to_owned(),
            oid: tip
        }
    );
    assert!(!f.git_dir().join("refs/bisect").join("bad").exists());
}

#[test]
fn a_bisect_run_in_a_terminal_reads_the_same() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    f.git(&["bisect", "start"]).unwrap();

    let waiting = bisect(&open(&f));
    assert_eq!(waiting.start, "main");
    assert_eq!((waiting.bad.as_deref(), waiting.good.len()), (None, 0));

    f.git(&["bisect", "bad"]).unwrap();
    f.git(&["bisect", "good", &commit(&f, 0)]).unwrap();

    let state = bisect(&open(&f));
    assert_eq!(state.bad.as_deref(), Some(commit(&f, COMMITS - 1).as_str()));
    assert_eq!(state.good, [commit(&f, 0)]);
    assert_eq!(state.current, Some(f.oid("HEAD").unwrap()));
}

#[test]
fn a_bisect_with_its_own_terms_is_marked_in_them() {
    let f = test_fixtures::linear(COMMITS).unwrap();
    f.git(&["bisect", "start", "--term-new=broken", "--term-old=fixed"])
        .unwrap();
    f.git(&["bisect", "broken"]).unwrap();
    let repo = open(&f);

    repo.bisect_mark(BisectMark::Good, Some(&commit(&f, 0)))
        .unwrap();

    let state = bisect(&repo);
    assert_eq!(
        (state.terms.bad.as_str(), state.terms.good.as_str()),
        ("broken", "fixed")
    );
    assert_eq!(state.bad.as_deref(), Some(commit(&f, COMMITS - 1).as_str()));
    assert_eq!(state.good, [commit(&f, 0)]);
}

#[test]
fn marking_without_a_bisect_is_refused_before_git_runs() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let refused = repo.bisect_mark(BisectMark::Good, None);

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "got {refused:?}"
    );
    assert!(matches!(
        repo.bisect_reset(),
        Err(GitError::InvalidState(_))
    ));
}

#[test]
fn a_second_bisect_is_refused_while_one_is_in_progress() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    repo.bisect_start("HEAD", None).unwrap();

    let refused = repo.bisect_start("HEAD", None);

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "got {refused:?}"
    );
}

#[test]
fn the_same_commit_cannot_be_good_and_bad() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let refused = repo.bisect_start("HEAD", Some("main"));

    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "got {refused:?}"
    );
    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

// `refs/bisect/*` and the BISECT_* files belong to each worktree: a linked one keeps them
// in its own git directory, and the main one knows nothing of its bisect.
#[test]
fn a_linked_worktree_bisects_on_its_own() {
    let f = test_fixtures::with_worktree().unwrap();
    let listed = f.git(&["worktree", "list", "--porcelain"]).unwrap();
    let linked = listed
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .nth(1)
        .unwrap()
        .to_owned();
    let linked = std::path::Path::new(&linked);
    f.git_in(linked, &["bisect", "start", "HEAD", "HEAD~2", "--"])
        .unwrap();

    let state = bisect(&RepoHandle::open(linked).unwrap());

    assert_eq!(state.start, "feature-wt");
    assert_eq!(state.bad, Some(f.oid("feature-wt").unwrap()));
    assert_eq!(state.good, [f.oid("feature-wt~2").unwrap()]);
    assert_eq!(open(&f).state().unwrap(), RepoState::Clean);
}
