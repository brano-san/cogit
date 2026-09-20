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
