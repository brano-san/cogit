// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn head_branch(repo: &RepoHandle) -> git_engine::Branch {
    repo.branches()
        .unwrap()
        .into_iter()
        .find(|b| b.is_head)
        .expect("a fixture always checks out a branch")
}

#[test]
fn a_branch_without_a_remote_has_no_upstream() {
    let f = test_fixtures::linear(2).unwrap();
    let branch = head_branch(&open(&f));

    assert!(branch.upstream.is_none());
    assert_eq!(branch.ahead, 0);
    assert_eq!(branch.behind, 0);
}

#[test]
fn a_tracked_branch_names_its_upstream() {
    let f = test_fixtures::with_remote().unwrap();
    let branch = head_branch(&open(&f));

    assert_eq!(branch.upstream.as_deref(), Some("origin/main"));
}

#[test]
fn divergence_is_counted_in_both_directions() {
    let f = test_fixtures::with_remote().unwrap();
    let branch = head_branch(&open(&f));

    assert_eq!(branch.ahead, 2, "two local commits the remote has not seen");
    assert_eq!(branch.behind, 1, "one remote commit not yet merged");
}

#[test]
fn a_branch_in_step_with_its_upstream_counts_zero() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
    let branch = head_branch(&open(&f));

    assert_eq!((branch.ahead, branch.behind), (0, 0));
}

#[test]
fn remote_branches_never_claim_an_upstream_of_their_own() {
    let f = test_fixtures::with_remote().unwrap();

    let remotes: Vec<git_engine::Branch> = open(&f)
        .branches()
        .unwrap()
        .into_iter()
        .filter(|b| b.kind == git_engine::BranchKind::Remote)
        .collect();

    assert!(!remotes.is_empty());
    assert!(remotes.iter().all(|b| b.upstream.is_none()));
}

/// `git rev-list --left-right --count local...upstream`.
fn git_counts(f: &test_fixtures::Fixture, local: &str, upstream: &str) -> (u32, u32) {
    let out = f
        .git(&[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{local}...{upstream}"),
        ])
        .unwrap();
    let (ahead, behind) = out.trim().split_once('\t').unwrap();
    (ahead.parse().unwrap(), behind.parse().unwrap())
}

// Counted from the merge base, a history rewritten on the server (filter-repo, an orphan
// branch) had none and showed 0/0, "in step", where git says the two have diverged.
#[test]
fn an_upstream_that_shares_no_history_counts_every_commit_on_both_sides() {
    let f = test_fixtures::with_remote().unwrap();
    let tree = f.oid("HEAD^{tree}").unwrap();
    let root = f
        .git_at(40, &["commit-tree", &tree, "-m", "rewritten root"])
        .unwrap();
    let tip = f
        .git_at(
            41,
            &["commit-tree", &tree, "-p", root.trim(), "-m", "rewritten"],
        )
        .unwrap();
    f.git(&["update-ref", "refs/remotes/origin/main", tip.trim()])
        .unwrap();

    let branch = head_branch(&open(&f));

    assert_eq!(
        (branch.ahead, branch.behind),
        git_counts(&f, "main", "origin/main")
    );
    assert_eq!(branch.behind, 2);
}

// A criss-cross merge has two merge bases; counting from the first one put the commits of
// the second on both sides.
#[test]
fn a_criss_cross_history_counts_as_git_does() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["branch", "other"]).unwrap();
    let mine = f.commit_file(10, "mine.txt", "m\n").unwrap();
    f.git(&["switch", "-q", "other"]).unwrap();
    let theirs = f.commit_file(11, "theirs.txt", "t\n").unwrap();
    f.merge(12, &[&mine], "theirs takes mine").unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.merge(13, &[&theirs], "mine takes theirs").unwrap();
    f.git(&["remote", "add", "origin", &f.path().to_string_lossy()])
        .unwrap();
    f.git(&["update-ref", "refs/remotes/origin/main", "other"])
        .unwrap();
    f.git(&["branch", "--set-upstream-to=origin/main", "main"])
        .unwrap();

    let branch = head_branch(&open(&f));

    assert_eq!(
        (branch.ahead, branch.behind),
        git_counts(&f, "main", "origin/main")
    );
    assert_eq!((branch.ahead, branch.behind), (1, 1));
}

#[test]
fn an_unborn_head_does_not_panic() {
    let f = test_fixtures::empty().unwrap();
    assert!(open(&f).branches().unwrap().is_empty());
}

#[test]
fn counting_does_not_spawn_a_process_per_branch() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::with_remote().unwrap();
    for i in 0..20 {
        f.git(&["branch", &format!("topic-{i}")]).unwrap();
    }

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.branches().unwrap();

    let commands = log.lock().unwrap().clone();
    assert!(
        commands.is_empty(),
        "reads go through gix; 500 branches must not mean 500 processes, got {commands:?}"
    );
}
