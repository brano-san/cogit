// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! One status walk feeds the Files list and the header counters (R-316).

use git_engine::RepoHandle;

#[test]
fn one_walk_gives_the_list_and_the_counters_that_working_state_would() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    std::fs::write(f.path().join("file1.txt"), "edited\n").unwrap();
    std::fs::create_dir(f.path().join("generated")).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(f.path().join("generated").join(name), "x\n").unwrap();
    }
    let repo = RepoHandle::open(f.path()).unwrap();
    repo.stage(&["file1.txt".to_owned()]).unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(files.state, repo.working_state().unwrap());
    // Each new file is a row in the list and one in the header.
    assert_eq!(files.state.status.untracked, 3);
    assert_eq!(files.state.status.unstaged, 1);
    assert_eq!(files.state.status.staged, 1);
}

#[test]
fn one_walk_names_the_conflicted_paths_like_working_state() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let files = repo.worktree_files().unwrap();
    assert!(!files.state.conflicted.is_empty());
    assert_eq!(files.state, repo.working_state().unwrap());
}
