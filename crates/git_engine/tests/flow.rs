#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Git-Flow over ordinary git commands. The `git-flow` extension is not a dependency:
//! every step here is a branch, a merge and a tag (M5 T5.5).

use git_engine::{FlowConfig, FlowKind, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// A repository with `main` and nothing else, which is what `init` starts from.
fn plain() -> test_fixtures::Fixture {
    test_fixtures::linear(2).unwrap()
}

fn started() -> test_fixtures::Fixture {
    let f = plain();
    open(&f).flow_init(&FlowConfig::default()).unwrap();
    f
}

fn branches(f: &test_fixtures::Fixture) -> String {
    f.git(&["branch", "--list", "--format=%(refname:short)"])
        .unwrap()
}

#[test]
fn a_fresh_repository_has_no_flow() {
    let f = plain();
    assert!(!open(&f).flow_status().unwrap().initialised);
}

#[test]
fn init_writes_the_config_the_extension_would() {
    let f = plain();
    open(&f).flow_init(&FlowConfig::default()).unwrap();

    let value = f.git(&["config", "gitflow.prefix.feature"]).unwrap();
    assert_eq!(value.trim(), "feature/");
}

#[test]
fn init_is_what_makes_the_status_say_initialised() {
    let f = started();
    assert!(open(&f).flow_status().unwrap().initialised);
}

#[test]
fn init_creates_the_develop_branch_when_there_is_none() {
    let f = started();
    assert!(branches(&f).lines().any(|line| line.trim() == "develop"));
}

#[test]
fn init_leaves_an_existing_develop_branch_where_it_is() {
    let f = plain();
    f.git(&["branch", "develop"]).unwrap();
    let before = f.git(&["rev-parse", "develop"]).unwrap();

    open(&f).flow_init(&FlowConfig::default()).unwrap();

    assert_eq!(f.git(&["rev-parse", "develop"]).unwrap(), before);
}

#[test]
fn the_prefixes_can_be_chosen() {
    let f = plain();
    let config = FlowConfig {
        feature: "f/".to_owned(),
        ..FlowConfig::default()
    };
    open(&f).flow_init(&config).unwrap();

    assert_eq!(open(&f).flow_status().unwrap().config.feature, "f/");
}

#[test]
fn the_status_reads_the_prefixes_back_from_the_config() {
    let f = started();
    let status = open(&f).flow_status().unwrap();
    assert_eq!(status.config.release, "release/");
    assert_eq!(status.config.develop, "develop");
}

#[test]
fn starting_a_feature_makes_a_branch_under_the_prefix() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();

    assert!(
        branches(&f)
            .lines()
            .any(|line| line.trim() == "feature/login")
    );
}

#[test]
fn starting_a_feature_checks_it_out() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();

    let head = f.git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    assert_eq!(head.trim(), "feature/login");
}

#[test]
fn a_feature_starts_from_develop() {
    let f = started();
    f.git(&["switch", "main"]).unwrap();
    std::fs::write(f.path().join("only-on-main.txt"), "x\n").unwrap();
    f.git(&["add", "--", "only-on-main.txt"]).unwrap();
    f.git(&["commit", "-m", "only on main"]).unwrap();

    open(&f).flow_start(FlowKind::Feature, "login").unwrap();

    assert!(!f.path().join("only-on-main.txt").exists());
}

#[test]
fn a_hotfix_starts_from_the_main_branch_not_from_develop() {
    let f = started();
    f.git(&["switch", "develop"]).unwrap();
    std::fs::write(f.path().join("only-on-develop.txt"), "x\n").unwrap();
    f.git(&["add", "--", "only-on-develop.txt"]).unwrap();
    f.git(&["commit", "-m", "only on develop"]).unwrap();

    open(&f).flow_start(FlowKind::Hotfix, "1.0.1").unwrap();

    assert!(!f.path().join("only-on-develop.txt").exists());
}

#[test]
fn the_status_lists_the_flow_branches_that_exist() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();

    let status = open(&f).flow_status().unwrap();
    assert!(
        status
            .branches
            .iter()
            .any(|branch| branch.kind == FlowKind::Feature && branch.name == "login"),
        "{:?}",
        status.branches
    );
}

#[test]
fn the_status_says_which_flow_branch_is_checked_out() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();

    let status = open(&f).flow_status().unwrap();
    assert!(status.branches.iter().any(|branch| branch.is_head));
}

#[test]
fn starting_a_feature_that_is_already_there_is_an_error() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();
    assert!(open(&f).flow_start(FlowKind::Feature, "login").is_err());
}

#[test]
fn a_nameless_branch_is_refused_before_anything_is_created() {
    let f = started();
    assert!(open(&f).flow_start(FlowKind::Feature, "   ").is_err());
}

#[test]
fn starting_anything_before_init_is_an_error() {
    let f = plain();
    assert!(open(&f).flow_start(FlowKind::Feature, "login").is_err());
}

#[test]
fn finishing_a_feature_folds_it_into_develop() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();
    std::fs::write(f.path().join("login.rs"), "fn login() {}\n").unwrap();
    f.git(&["add", "--", "login.rs"]).unwrap();
    f.git(&["commit", "-m", "add login"]).unwrap();

    open(&f)
        .flow_finish(FlowKind::Feature, "login", None)
        .unwrap();

    let log = f.git(&["log", "--oneline", "develop"]).unwrap();
    assert!(log.contains("add login"), "{log}");
}

#[test]
fn finishing_a_feature_deletes_its_branch() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();
    open(&f)
        .flow_finish(FlowKind::Feature, "login", None)
        .unwrap();

    assert!(
        !branches(&f)
            .lines()
            .any(|line| line.trim() == "feature/login")
    );
}

#[test]
fn finishing_a_feature_leaves_develop_checked_out() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();
    open(&f)
        .flow_finish(FlowKind::Feature, "login", None)
        .unwrap();

    let head = f.git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    assert_eq!(head.trim(), "develop");
}

#[test]
fn a_feature_merge_keeps_its_own_commit_so_the_branch_is_visible_afterwards() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "login").unwrap();
    std::fs::write(f.path().join("login.rs"), "fn login() {}\n").unwrap();
    f.git(&["add", "--", "login.rs"]).unwrap();
    f.git(&["commit", "-m", "add login"]).unwrap();

    open(&f)
        .flow_finish(FlowKind::Feature, "login", None)
        .unwrap();

    // `--no-ff`, or a finished feature is indistinguishable from commits made on develop.
    let parents = f
        .git(&["rev-list", "--parents", "-n", "1", "develop"])
        .unwrap();
    assert_eq!(parents.split_whitespace().count(), 3, "{parents}");
}

#[test]
fn finishing_a_release_reaches_both_the_main_branch_and_develop() {
    let f = started();
    open(&f).flow_start(FlowKind::Release, "1.0").unwrap();
    std::fs::write(f.path().join("notes.md"), "1.0\n").unwrap();
    f.git(&["add", "--", "notes.md"]).unwrap();
    f.git(&["commit", "-m", "release notes"]).unwrap();

    open(&f)
        .flow_finish(FlowKind::Release, "1.0", Some("1.0"))
        .unwrap();

    assert!(
        f.git(&["log", "--oneline", "main"])
            .unwrap()
            .contains("release notes")
    );
    assert!(
        f.git(&["log", "--oneline", "develop"])
            .unwrap()
            .contains("release notes")
    );
}

#[test]
fn finishing_a_release_tags_the_main_branch() {
    let f = started();
    open(&f).flow_start(FlowKind::Release, "1.0").unwrap();
    open(&f)
        .flow_finish(FlowKind::Release, "1.0", Some("1.0"))
        .unwrap();

    assert!(f.git(&["tag", "--list"]).unwrap().contains("1.0"));
}

#[test]
fn a_release_without_a_tag_name_is_not_tagged() {
    let f = started();
    open(&f).flow_start(FlowKind::Release, "1.0").unwrap();
    open(&f)
        .flow_finish(FlowKind::Release, "1.0", None)
        .unwrap();

    assert!(f.git(&["tag", "--list"]).unwrap().trim().is_empty());
}

#[test]
fn a_hotfix_finishes_the_same_way_a_release_does() {
    let f = started();
    open(&f).flow_start(FlowKind::Hotfix, "1.0.1").unwrap();
    std::fs::write(f.path().join("fix.rs"), "fixed\n").unwrap();
    f.git(&["add", "--", "fix.rs"]).unwrap();
    f.git(&["commit", "-m", "the fix"]).unwrap();

    open(&f)
        .flow_finish(FlowKind::Hotfix, "1.0.1", Some("1.0.1"))
        .unwrap();

    assert!(
        f.git(&["log", "--oneline", "main"])
            .unwrap()
            .contains("the fix")
    );
    assert!(
        f.git(&["log", "--oneline", "develop"])
            .unwrap()
            .contains("the fix")
    );
}

#[test]
fn finishing_a_branch_that_is_not_there_is_an_error() {
    let f = started();
    assert!(
        open(&f)
            .flow_finish(FlowKind::Feature, "nope", None)
            .is_err()
    );
}

#[test]
fn a_conflicting_finish_leaves_the_merge_for_the_user_rather_than_pretending() {
    let f = started();
    open(&f).flow_start(FlowKind::Feature, "one").unwrap();
    std::fs::write(f.path().join("shared.txt"), "from the feature\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.git(&["commit", "-m", "feature side"]).unwrap();

    f.git(&["switch", "develop"]).unwrap();
    std::fs::write(f.path().join("shared.txt"), "from develop\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.git(&["commit", "-m", "develop side"]).unwrap();

    assert!(
        open(&f)
            .flow_finish(FlowKind::Feature, "one", None)
            .is_err()
    );
    // The branch is still there, so the user can finish it once the conflict is settled.
    assert!(
        branches(&f)
            .lines()
            .any(|line| line.trim() == "feature/one")
    );
}
