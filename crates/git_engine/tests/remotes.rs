// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The menu of a remote in Branches: Properties, Rename, Delete, Fetch More, Set Depth
//! (#19 of 25.09) and the per-remote switch of the background check.

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn no_token(_url: &str) -> Option<String> {
    None
}

fn origin_url(f: &test_fixtures::Fixture) -> String {
    f.git(&["config", "remote.origin.url"])
        .unwrap()
        .trim()
        .to_owned()
}

fn refs_under(f: &test_fixtures::Fixture, prefix: &str) -> Vec<String> {
    f.git(&["for-each-ref", "--format=%(refname)", prefix])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_remote_reads_back_as_its_config_says() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&[
        "config",
        "remote.origin.pushurl",
        "https://example.com/push.git",
    ])
    .unwrap();

    let info = open(&f).remote_info("origin").unwrap();

    assert_eq!(info.name, "origin");
    assert_eq!(info.url.as_deref(), Some(origin_url(&f).as_str()));
    assert_eq!(
        info.push_url.as_deref(),
        Some("https://example.com/push.git")
    );
    assert!(info.background_fetch, "on unless turned off");
    assert!(!info.shallow);
}

#[test]
fn an_unknown_remote_is_refused_by_name() {
    let f = test_fixtures::linear(1).unwrap();
    let err = open(&f).remote_info("nowhere").unwrap_err();
    assert!(err.to_string().contains("nowhere"), "{err}");
}

// The switch lives in the remote's own section: `git remote rename` carries it, `git
// remote remove` takes it away, and "on" leaves nothing behind in the config.
#[test]
fn the_background_check_is_switched_per_remote_and_follows_a_rename() {
    let f = test_fixtures::with_remote().unwrap();

    open(&f).set_background_fetch("origin", false).unwrap();
    assert!(!open(&f).remote_info("origin").unwrap().background_fetch);

    open(&f).rename_remote("origin", "mine").unwrap();
    assert!(!open(&f).remote_info("mine").unwrap().background_fetch);

    open(&f).set_background_fetch("mine", true).unwrap();
    assert!(open(&f).remote_info("mine").unwrap().background_fetch);
    assert!(
        f.git(&["config", "--get-regexp", "cogit"])
            .unwrap_or_default()
            .trim()
            .is_empty(),
        "on is the default, so nothing is written for it"
    );
}

#[test]
fn a_rename_moves_the_remote_branches_and_the_upstreams() {
    let f = test_fixtures::with_remote().unwrap();

    open(&f).rename_remote("origin", "upstream").unwrap();

    assert!(refs_under(&f, "refs/remotes/origin").is_empty());
    assert!(
        refs_under(&f, "refs/remotes/upstream").contains(&"refs/remotes/upstream/main".to_owned())
    );
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "main@{upstream}"])
            .unwrap()
            .trim(),
        "upstream/main"
    );
}

#[test]
fn a_deleted_remote_takes_its_remote_branches_with_it() {
    let f = test_fixtures::with_remote().unwrap();

    open(&f).remove_remote("origin").unwrap();

    assert!(open(&f).remotes().unwrap().is_empty());
    assert!(refs_under(&f, "refs/remotes/origin").is_empty());
}

#[test]
fn a_new_url_is_written_and_one_that_reads_as_an_option_is_refused() {
    let f = test_fixtures::with_remote().unwrap();

    open(&f)
        .set_remote_url("origin", "https://example.com/moved.git")
        .unwrap();
    assert_eq!(origin_url(&f), "https://example.com/moved.git");

    assert!(open(&f).set_remote_url("origin", "--push").is_err());
    assert_eq!(origin_url(&f), "https://example.com/moved.git");
}

/// `origin` fetches only `main`, as a single-branch clone does; the server has `topic` too.
fn narrow(f: &test_fixtures::Fixture) {
    f.git(&["push", "origin", "HEAD:refs/heads/topic"]).unwrap();
    f.git(&[
        "config",
        "remote.origin.fetch",
        "+refs/heads/main:refs/remotes/origin/main",
    ])
    .unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/topic"])
        .unwrap();
}

// SmartGit: "Use Remote | Fetch More if a remote contains branches which are not yet
// available in the local repository" — after a narrow clone in particular.
#[test]
fn fetch_more_brings_the_branches_the_refspec_leaves_out_and_says_when_nothing_came() {
    let f = test_fixtures::with_remote().unwrap();
    narrow(&f);

    let first = open(&f).fetch_more("origin", no_token, |_| {}).unwrap();
    let second = open(&f).fetch_more("origin", no_token, |_| {}).unwrap();

    assert!(first, "topic came");
    assert!(
        refs_under(&f, "refs/remotes/origin").contains(&"refs/remotes/origin/topic".to_owned())
    );
    assert!(!second, "nothing new the second time");
}

#[test]
fn set_depth_refuses_a_full_clone_and_deepens_a_shallow_one() {
    let f = test_fixtures::with_remote().unwrap();
    assert!(open(&f).fetch_depth("origin", 1, no_token, |_| {}).is_err());

    let dir = tempfile::TempDir::new().unwrap();
    // A plain path clones locally, and a local clone ignores --depth.
    let path = origin_url(&f).replace('\\', "/");
    let url = format!("file:///{}", path.trim_start_matches('/'));
    let clone = dir.path().join("shallow");
    f.git(&["clone", "--depth=1", &url, &clone.to_string_lossy()])
        .unwrap();
    let count = || {
        f.git_in(&clone, &["rev-list", "--count", "HEAD"])
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap()
    };
    assert_eq!(count(), 1);
    let shallow = RepoHandle::open(&clone).unwrap();
    assert!(shallow.remote_info("origin").unwrap().shallow);

    shallow.fetch_depth("origin", 2, no_token, |_| {}).unwrap();

    assert_eq!(count(), 2);
}

#[test]
fn a_depth_of_nothing_is_refused_before_git_runs() {
    let f = test_fixtures::with_remote().unwrap();
    let err = open(&f)
        .fetch_depth("origin", 0, no_token, |_| {})
        .unwrap_err();
    assert!(
        matches!(err, git_engine::GitError::InvalidState(_)),
        "{err:?}"
    );
}
