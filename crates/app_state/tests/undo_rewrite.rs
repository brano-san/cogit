// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use git_engine::GitError;

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn a_fresh_repository_has_no_orig_head_and_the_undo_says_why() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["update-ref", "-d", "ORIG_HEAD"]).unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite_info(repo).unwrap().is_none());
    let err = state.undo_rewrite(repo).unwrap_err();

    assert!(
        matches!(err, GitError::InvalidState(ref m) if m.contains("ORIG_HEAD")),
        "{err:?}"
    );
}

#[test]
fn a_merge_is_undone_to_where_the_branch_was() {
    let f = test_fixtures::branched().unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    let (state, repo) = open(&f);

    let info = state.undo_rewrite_info(repo).unwrap().unwrap();
    assert_eq!(info.orig, before);
    assert!(!info.dirty);
    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn a_hard_reset_is_undone() {
    let f = test_fixtures::linear(3).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~2"]).unwrap();
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), tip);
}

#[test]
fn a_rebase_is_undone() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "dev"]).unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["rebase", "main"]).unwrap();
    assert_ne!(f.oid("HEAD").unwrap(), before);
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn local_changes_survive_the_undo_and_are_reported_before_it() {
    let f = test_fixtures::branched().unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    std::fs::write(f.path().join("base.txt"), "staged edit\n").unwrap();
    f.git(&["add", "base.txt"]).unwrap();
    std::fs::write(f.path().join("scratch.txt"), "untracked\n").unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite_info(repo).unwrap().unwrap().dirty);
    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
    let read = |name: &str| std::fs::read_to_string(f.path().join(name)).unwrap();
    assert_eq!(read("base.txt"), "staged edit\n");
    assert_eq!(read("scratch.txt"), "untracked\n");
    // Staged edits may come back unstaged, but are never lost.
    assert!(
        f.git(&["status", "--porcelain"])
            .unwrap()
            .contains("base.txt")
    );
}

#[test]
fn an_undo_that_would_overwrite_local_work_changes_nothing() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    let merged = f.oid("HEAD").unwrap();
    std::fs::write(f.path().join("dev-1.txt"), "my edit\n").unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite(repo).is_err());

    assert_eq!(f.oid("HEAD").unwrap(), merged);
    assert_eq!(
        std::fs::read_to_string(f.path().join("dev-1.txt")).unwrap(),
        "my edit\n"
    );
}

#[test]
fn the_undo_is_in_the_journal_and_the_journal_undo_still_works() {
    let f = test_fixtures::linear(3).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();
    assert_eq!(f.oid("HEAD").unwrap(), tip);
    state.undo_last(repo).unwrap();

    assert_ne!(f.oid("HEAD").unwrap(), tip);
}

#[test]
fn compare_before_and_after_rewrite_reads_orig_head_without_the_list() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["commit", "-q", "--amend", "-m", "reworded"])
        .unwrap();
    let (state, repo) = open(&f);

    let out = state.range_diff(repo, "ORIG_HEAD", "HEAD").unwrap();

    assert!(out.stdout.contains("reworded"), "{}", out.stdout);
}

#[test]
fn undoing_a_fix_up_keeps_the_folded_changes_as_a_commit_of_their_own() {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.path().join("fix.txt"), "the fix\n").unwrap();
    f.git(&["add", "--", "fix.txt"]).unwrap();
    let (state, repo) = open(&f);

    state.fixup_into(repo, "HEAD~1").unwrap();
    assert!(
        f.git(&["ls-tree", "--name-only", "HEAD~1"])
            .unwrap()
            .contains("fix.txt")
    );
    state.undo_last(repo).unwrap();

    let subjects = f.git(&["log", "--format=%s"]).unwrap();
    assert!(
        subjects.starts_with("fixup! commit 1\ncommit 2\n"),
        "{subjects}"
    );
    assert!(
        f.git(&["ls-tree", "--name-only", "HEAD"])
            .unwrap()
            .contains("fix.txt")
    );
}

fn rebase_onto(onto: &str) -> git_engine::RebaseOptions {
    git_engine::RebaseOptions {
        onto: onto.to_owned(),
        autostash: false,
        upstream: None,
    }
}

/// dev stacked on feat-1 (dev~1), both off main's first commit; `rebase.updateRefs` on.
fn stacked() -> test_fixtures::Fixture {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "dev"]).unwrap();
    f.git(&["branch", "feat-1", "dev~1"]).unwrap();
    f.git(&["config", "rebase.updateRefs", "true"]).unwrap();
    f
}

#[test]
fn undoing_a_rebase_with_update_refs_puts_the_stacked_branches_back() {
    let f = stacked();
    let (dev, feat) = (f.oid("dev").unwrap(), f.oid("feat-1").unwrap());
    let (state, repo) = open(&f);

    state.rebase(repo, &rebase_onto("main")).unwrap();
    assert_ne!(
        f.oid("feat-1").unwrap(),
        feat,
        "git moved the stacked branch"
    );
    assert!(
        state.safety_log()[0]
            .description
            .contains("also moved feat-1")
    );
    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("dev").unwrap(), dev);
    assert_eq!(f.oid("feat-1").unwrap(), feat);
}

#[test]
fn the_undo_of_a_stacked_rebase_is_undone_with_its_branches() {
    let f = stacked();
    let (state, repo) = open(&f);
    state.rebase(repo, &rebase_onto("main")).unwrap();
    let (dev, feat) = (f.oid("dev").unwrap(), f.oid("feat-1").unwrap());

    state.undo_last(repo).unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("dev").unwrap(), dev);
    assert_eq!(f.oid("feat-1").unwrap(), feat);
}

#[test]
fn a_stacked_branch_moved_since_refuses_the_undo_and_nothing_moves() {
    let f = stacked();
    let (state, repo) = open(&f);
    state.rebase(repo, &rebase_onto("main")).unwrap();
    let rebased = f.oid("dev").unwrap();
    f.git(&["branch", "-f", "feat-1", "main"]).unwrap();

    let err = state.undo_last(repo).unwrap_err();

    assert!(
        matches!(&err, GitError::InvalidState(why) if why.contains("feat-1")),
        "{err:?}"
    );
    assert_eq!(f.oid("dev").unwrap(), rebased);
    assert_eq!(f.oid("feat-1").unwrap(), f.oid("main").unwrap());
}

#[test]
fn a_rebase_without_update_refs_leaves_the_other_branches_out_of_the_undo() {
    let f = stacked();
    f.git(&["config", "rebase.updateRefs", "false"]).unwrap();
    let feat = f.oid("feat-1").unwrap();
    let (state, repo) = open(&f);

    state.rebase(repo, &rebase_onto("main")).unwrap();
    f.git(&["branch", "-f", "feat-1", "main"]).unwrap();
    state.undo_last(repo).unwrap();

    assert_ne!(
        f.oid("feat-1").unwrap(),
        feat,
        "not the rebase's to put back"
    );
    assert_eq!(f.oid("feat-1").unwrap(), f.oid("main").unwrap());
}

#[test]
fn undoing_a_split_with_update_refs_puts_the_stacked_branches_back() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("a.txt", "a\n").unwrap();
    f.write_file("b.txt", "b\n").unwrap();
    f.git(&["add", "a.txt", "b.txt"]).unwrap();
    let split = f.commit_staged(5, "two files").unwrap();
    f.commit_file(6, "c.txt", "c\n").unwrap();
    f.git(&["branch", "feat-1"]).unwrap();
    f.commit_file(7, "d.txt", "d\n").unwrap();
    f.git(&["config", "rebase.updateRefs", "true"]).unwrap();
    let (head, feat) = (f.oid("HEAD").unwrap(), f.oid("feat-1").unwrap());
    let (state, repo) = open(&f);

    state
        .split_off(repo, &split, &["a.txt".to_owned()], "a alone", true)
        .unwrap();
    assert_ne!(
        f.oid("feat-1").unwrap(),
        feat,
        "git moved the stacked branch"
    );
    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), head);
    assert_eq!(f.oid("feat-1").unwrap(), feat);
}

#[test]
fn a_stacked_rebase_finished_by_continue_undoes_with_its_branches() {
    let f = stacked();
    f.git(&["checkout", "-q", "main"]).unwrap();
    f.commit_file(8, "dev-2.txt", "main two\n").unwrap();
    f.git(&["checkout", "-q", "dev"]).unwrap();
    let (dev, feat) = (f.oid("dev").unwrap(), f.oid("feat-1").unwrap());
    let (state, repo) = open(&f);

    assert!(state.rebase(repo, &rebase_onto("main")).is_err());
    f.write_file("dev-2.txt", "resolved\n").unwrap();
    f.git(&["add", "dev-2.txt"]).unwrap();
    state.continue_operation(repo).unwrap();
    assert_ne!(
        f.oid("feat-1").unwrap(),
        feat,
        "git moved the stacked branch"
    );
    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("dev").unwrap(), dev);
    assert_eq!(f.oid("feat-1").unwrap(), feat);
}

/// A rebase with `rebase.updateRefs` run by any tool moves the stacked branches too; Undo
/// Last Rebase puts them all back, and names them before it does (R-948).
#[test]
fn undoing_a_rebase_with_update_refs_puts_every_moved_branch_back() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "-b", "feat-1", "dev"]).unwrap();
    f.git(&["checkout", "-q", "-b", "feat-2"]).unwrap();
    f.commit_file(5, "feat-2.txt", "two\n").unwrap();
    let (feat_1, feat_2, dev) = (
        f.oid("feat-1").unwrap(),
        f.oid("feat-2").unwrap(),
        f.oid("dev").unwrap(),
    );
    f.git(&["rebase", "--update-refs", "main"]).unwrap();
    assert_ne!(f.oid("feat-1").unwrap(), feat_1, "the stack moved");
    let (state, repo) = open(&f);

    let info = state.undo_rewrite_info(repo).unwrap().unwrap();
    let mut stacked = info.stacked.clone();
    stacked.sort();
    assert_eq!(stacked, ["dev", "feat-1"]);
    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("feat-2").unwrap(), feat_2);
    assert_eq!(f.oid("feat-1").unwrap(), feat_1);
    assert_eq!(f.oid("dev").unwrap(), dev);
}

/// A branch moved again after the rebase is not the rebase's to take back.
#[test]
fn a_stacked_branch_moved_since_the_rebase_stays() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "-b", "feat-1", "dev"]).unwrap();
    f.git(&["checkout", "-q", "-b", "feat-2"]).unwrap();
    f.commit_file(5, "feat-2.txt", "two\n").unwrap();
    f.git(&["rebase", "--update-refs", "main"]).unwrap();
    f.git(&["branch", "--force", "feat-1", "main"]).unwrap();
    let (state, repo) = open(&f);

    let info = state.undo_rewrite_info(repo).unwrap().unwrap();
    assert_eq!(info.stacked, ["dev"]);
}
