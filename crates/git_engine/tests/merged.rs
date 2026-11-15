// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_branch_that_forked_off_is_not_in_head() {
    let f = test_fixtures::branched().unwrap();
    assert!(!open(&f).is_merged_into_head("dev").unwrap());
}

#[test]
fn an_ancestor_of_head_is_already_in_it() {
    let f = test_fixtures::branched().unwrap();
    assert!(open(&f).is_merged_into_head("main~1").unwrap());
}

#[test]
fn head_itself_counts_as_merged() {
    let f = test_fixtures::branched().unwrap();
    let head = f.oid("HEAD").unwrap();
    assert!(open(&f).is_merged_into_head(&head).unwrap());
}

#[test]
fn a_branch_merged_earlier_is_in_head() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    assert!(open(&f).is_merged_into_head("dev").unwrap());
}

/// With a commit-graph the answer comes from the generation-pruned walk; it must stay git's.
#[test]
fn with_a_commit_graph_every_commit_is_judged_as_git_judges_it() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev~1", "dev~1"])
        .unwrap();
    f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    let repo = open(&f);
    let all = f.git(&["rev-list", "--all"]).unwrap();
    assert_eq!(all.lines().count(), 5);
    for oid in all.lines() {
        let git_says = f.git(&["merge-base", "--is-ancestor", oid, "HEAD"]).is_ok();
        assert_eq!(repo.is_merged_into_head(oid).unwrap(), git_says, "{oid}");
    }
}

#[test]
fn an_unknown_revision_is_an_error() {
    let f = test_fixtures::branched().unwrap();
    assert!(open(&f).is_merged_into_head("no-such-branch").is_err());
}

/// `origin` holds `alive`, `gone-merged` and `gone-unmerged`; the last two are then
/// deleted there and pruned here, as a teammate's cleanup after merging would leave them.
fn after_cleanup_on_the_remote() -> test_fixtures::Fixture {
    let f = test_fixtures::with_remote().unwrap();
    for name in ["alive", "gone-merged", "local-only"] {
        f.git(&["branch", name, "main~1"]).unwrap();
    }
    f.git(&["switch", "-c", "gone-unmerged"]).unwrap();
    f.commit_file(40, "unmerged.txt", "only here\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    for name in ["alive", "gone-merged", "gone-unmerged"] {
        f.git(&["push", "--set-upstream", "origin", name]).unwrap();
    }
    f.git(&["push", "origin", "--delete", "gone-merged", "gone-unmerged"])
        .unwrap();
    f.git(&["fetch", "--prune", "origin"]).unwrap();
    f
}

#[test]
fn a_merged_branch_whose_upstream_was_deleted_is_offered_for_deletion() {
    let f = after_cleanup_on_the_remote();
    assert_eq!(gone(&f), ["gone-merged"]);
}

#[test]
fn a_branch_whose_upstream_still_exists_is_kept() {
    let f = after_cleanup_on_the_remote();
    let names = gone(&f);
    assert!(!names.contains(&"alive".to_owned()), "{names:?}");
}

#[test]
fn a_branch_never_pushed_is_kept() {
    let f = after_cleanup_on_the_remote();
    let names = gone(&f);
    assert!(!names.contains(&"local-only".to_owned()), "{names:?}");
}

#[test]
fn a_branch_with_commits_head_lacks_is_kept_even_when_gone() {
    let f = after_cleanup_on_the_remote();
    let names = gone(&f);
    assert!(!names.contains(&"gone-unmerged".to_owned()), "{names:?}");
}

#[test]
fn the_checked_out_branch_is_never_offered() {
    let f = after_cleanup_on_the_remote();
    f.git(&["switch", "gone-merged"]).unwrap();
    assert!(gone(&f).is_empty());
}

#[test]
fn a_branch_checked_out_in_another_worktree_is_kept() {
    let f = after_cleanup_on_the_remote();
    let elsewhere = f.path().join("elsewhere");
    f.git(&[
        "worktree",
        "add",
        &elsewhere.to_string_lossy(),
        "gone-merged",
    ])
    .unwrap();
    assert!(gone(&f).is_empty());
}

fn gone(f: &test_fixtures::Fixture) -> Vec<String> {
    let found = open(f)
        .merged_gone_branches(&mut git_engine::MergeCache::default())
        .unwrap();
    found.into_iter().map(|branch| branch.name).collect()
}

fn id(f: &test_fixtures::Fixture, rev: &str) -> git_engine::ObjectId {
    git_engine::ObjectId::from_hex(f.oid(rev).unwrap().as_bytes()).unwrap()
}

/// How HEAD took in each of `revs` (F-949).
fn kinds_within(f: &test_fixtures::Fixture, revs: &[&str], cap: usize) -> Vec<Option<MergeKind>> {
    let tips: Vec<_> = revs.iter().map(|rev| id(f, rev)).collect();
    let mut cache = git_engine::MergeCache::default();
    open(f)
        .merge_kinds(id(f, "HEAD"), &tips, cap, &mut cache, &|| false)
        .unwrap()
}

fn kind(f: &test_fixtures::Fixture, rev: &str) -> MergeKind {
    kinds_within(f, &[rev], git_engine::MERGE_SCAN_CAP)[0].unwrap()
}

use git_engine::MergeKind;

#[test]
fn a_squash_merged_branch_is_merged_by_squash() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--squash", "dev"]).unwrap();
    f.commit_staged(10, "squash dev").unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::Squash);
}

#[test]
fn a_branch_whose_commits_were_picked_onto_head_is_merged_by_rebase() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["cherry-pick", "dev~1", "dev"]).unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::Rebase);
}

#[test]
fn a_branch_with_one_commit_not_picked_is_not_merged() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["cherry-pick", "dev~1"]).unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::NotMerged);
}

#[test]
fn a_branch_whose_commit_was_amended_on_head_is_not_merged() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["cherry-pick", "dev~1"]).unwrap();
    f.git(&["cherry-pick", "--no-commit", "dev"]).unwrap();
    f.write_file("dev-2.txt", "dev two, amended\n").unwrap();
    f.git(&["add", "dev-2.txt"]).unwrap();
    f.commit_staged(10, "amended").unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::NotMerged);
}

#[test]
fn a_branch_merged_by_a_merge_commit_stays_merged() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::Merged);
}

#[test]
fn a_branch_never_taken_in_is_not_merged() {
    let f = test_fixtures::branched().unwrap();
    assert_eq!(kind(&f, "dev"), MergeKind::NotMerged);
}

#[test]
fn past_the_cap_on_the_target_side_it_is_not_checked() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--squash", "dev"]).unwrap();
    f.commit_staged(10, "squash dev").unwrap();
    // HEAD has two commits since the merge base: `commit 1` and the squash.
    assert_eq!(kinds_within(&f, &["dev"], 2)[0], Some(MergeKind::Squash));
    assert_eq!(
        kinds_within(&f, &["dev"], 1)[0],
        Some(MergeKind::NotChecked)
    );
}

#[test]
fn past_the_cap_on_the_branch_side_it_is_not_checked() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.commit_file(5, "dev-3.txt", "dev three\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["merge", "--squash", "dev"]).unwrap();
    f.commit_staged(10, "squash dev").unwrap();
    // Two commits on HEAD's side, three on dev's.
    assert_eq!(kinds_within(&f, &["dev"], 3)[0], Some(MergeKind::Squash));
    assert_eq!(
        kinds_within(&f, &["dev"], 2)[0],
        Some(MergeKind::NotChecked)
    );
}

#[test]
fn several_branches_are_judged_in_one_pass_and_the_cache_spares_git_the_second_time() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["branch", "picked", "dev~1"]).unwrap();
    f.git(&["branch", "old", "main"]).unwrap();
    f.git(&["merge", "--squash", "dev"]).unwrap();
    f.commit_staged(10, "squash dev").unwrap();
    let repo = open(&f);
    let tips = [id(&f, "dev"), id(&f, "picked"), id(&f, "old")];
    let mut cache = git_engine::MergeCache::default();

    let before = git_engine::children::spawned();
    let first = repo
        .merge_kinds(id(&f, "HEAD"), &tips, 100, &mut cache, &|| false)
        .unwrap();
    let spawned = git_engine::children::spawned() - before;
    assert_eq!(
        first,
        [
            Some(MergeKind::Squash),
            Some(MergeKind::NotMerged),
            Some(MergeKind::Merged)
        ]
    );
    assert_eq!(
        spawned, 2,
        "one diff-tree and one patch-id for every branch"
    );

    let before = git_engine::children::spawned();
    let again = repo
        .merge_kinds(id(&f, "HEAD"), &tips, 100, &mut cache, &|| false)
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(git_engine::children::spawned(), before);
}

#[test]
fn a_cancelled_scan_leaves_the_rest_unjudged() {
    let f = test_fixtures::branched().unwrap();
    let mut cache = git_engine::MergeCache::default();
    let found = open(&f)
        .merge_kinds(id(&f, "HEAD"), &[id(&f, "dev")], 100, &mut cache, &|| true)
        .unwrap();
    assert_eq!(found, [None]);
}

#[test]
fn branch_merges_lists_only_merged_local_branches_with_their_kind() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["branch", "picked", "dev~1"]).unwrap();
    f.git(&["branch", "behind", "main~1"]).unwrap();
    f.git(&["cherry-pick", "dev~1"]).unwrap();
    let found = open(&f)
        .branch_merges(&mut git_engine::MergeCache::default(), &|| false)
        .unwrap();
    let named: Vec<(&str, MergeKind)> = found
        .iter()
        .map(|branch| (branch.name.as_str(), branch.kind))
        .collect();
    assert_eq!(
        named,
        [("behind", MergeKind::Merged), ("picked", MergeKind::Rebase)]
    );
    assert_eq!(found[1].oid, f.oid("picked").unwrap());
}

#[test]
fn a_squash_merged_branch_whose_upstream_was_deleted_is_offered_with_its_kind() {
    let f = after_cleanup_on_the_remote();
    f.git(&["switch", "gone-unmerged"]).unwrap();
    f.commit_file(41, "unmerged-2.txt", "also here\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["merge", "--squash", "gone-unmerged"]).unwrap();
    f.commit_staged(50, "squash gone-unmerged").unwrap();
    let found = open(&f)
        .merged_gone_branches(&mut git_engine::MergeCache::default())
        .unwrap();
    let named: Vec<(&str, MergeKind)> = found
        .iter()
        .map(|branch| (branch.name.as_str(), branch.kind))
        .collect();
    assert_eq!(
        named,
        [
            ("gone-merged", MergeKind::Merged),
            ("gone-unmerged", MergeKind::Squash)
        ]
    );
}
