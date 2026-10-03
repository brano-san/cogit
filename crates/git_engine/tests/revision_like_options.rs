#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{MergeOptions, RebaseOptions, RepoHandle};

fn diverged() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(10, "theirs.txt", "theirs\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(11, "ours.txt", "ours\n").unwrap();
    f
}

// A tag from a foreign repository can be named like an option.
#[test]
fn a_tag_named_like_an_option_is_not_run_as_a_rebase_option() {
    let f = diverged();
    f.git(&["update-ref", "refs/tags/--exec=touch${IFS}pwned", "theirs"])
        .unwrap();
    // With an upstream, `rebase --exec=…` replays main's commit and runs the command after
    // it; without one it fails on "no tracking information" before running anything.
    f.git(&["branch", "--set-upstream-to=theirs", "main"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.rebase(&RebaseOptions {
        onto: "--exec=touch${IFS}pwned".to_owned(),
        autostash: false,
    })
    .unwrap();

    assert!(!f.path().join("pwned").exists());
    assert!(f.path().join("theirs.txt").exists(), "rebased onto the tag");
}

#[test]
fn a_tag_named_like_an_option_is_not_a_merge_option() {
    let f = diverged();
    f.git(&["update-ref", "refs/tags/--squash", "theirs"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    repo.merge(&MergeOptions {
        source: "--squash".to_owned(),
        no_fast_forward: true,
        squash: false,
        message: None,
    })
    .unwrap();

    let parents = f.git(&["rev-list", "--parents", "-1", "HEAD"]).unwrap();
    assert_eq!(parents.split_whitespace().count(), 3, "{parents}");
}
