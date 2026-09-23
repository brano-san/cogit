// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Remote ▸ LFS (#46).

use git_engine::{GitError, LfsOp, RepoHandle, lfs_version, lfs_version_from};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// The machine running the suite may not have Git LFS; those tests then have nothing to try.
fn lfs_installed() -> bool {
    std::process::Command::new("git")
        .args(["lfs", "version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_version_line_is_what_git_lfs_printed() {
    let found = lfs_version_from(
        Some(0),
        "git-lfs/3.7.0 (GitHub; windows amd64; go 1.24.4; git 92dddf56)\n",
    );
    assert_eq!(
        found.as_deref(),
        Some("git-lfs/3.7.0 (GitHub; windows amd64; go 1.24.4; git 92dddf56)")
    );
}

/// What git says when there is no `git-lfs` on the PATH.
#[test]
fn a_git_without_lfs_reports_none() {
    assert_eq!(lfs_version_from(Some(1), ""), None);
    assert_eq!(lfs_version_from(None, "git-lfs/3.7.0"), None);
    assert_eq!(lfs_version_from(Some(0), "   \n"), None);
}

#[test]
fn the_answer_matches_the_git_on_this_machine() {
    assert_eq!(lfs_version().is_some(), lfs_installed());
}

#[test]
fn install_sets_the_filters_up_in_this_repository_only() {
    if !lfs_installed() {
        return;
    }
    let f = test_fixtures::linear(1).unwrap();

    open(&f).lfs_op(&LfsOp::Install).unwrap();

    let local = f
        .git(&["config", "--local", "--get", "filter.lfs.clean"])
        .unwrap();
    assert!(local.contains("git-lfs clean"), "{local}");
    assert!(f.git_dir().join("hooks/pre-push").is_file());
}

#[test]
fn track_writes_the_pattern_to_gitattributes() {
    if !lfs_installed() {
        return;
    }
    let f = test_fixtures::linear(1).unwrap();
    open(&f).lfs_op(&LfsOp::Install).unwrap();

    open(&f)
        .lfs_op(&LfsOp::Track {
            pattern: "*.psd".to_owned(),
        })
        .unwrap();

    let attributes = std::fs::read_to_string(f.path().join(".gitattributes")).unwrap();
    assert!(attributes.contains("*.psd filter=lfs"), "{attributes}");
}

#[test]
fn a_pattern_that_reads_as_an_option_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    for pattern in ["", " ", "--global"] {
        let refused = open(&f).lfs_op(&LfsOp::Track {
            pattern: pattern.to_owned(),
        });
        assert!(
            matches!(refused, Err(GitError::InvalidState(_))),
            "{pattern}: {refused:?}"
        );
    }
}

#[test]
fn lock_needs_a_file() {
    let f = test_fixtures::linear(1).unwrap();
    let refused = open(&f).lfs_op(&LfsOp::Lock { paths: Vec::new() });
    assert!(
        matches!(refused, Err(GitError::InvalidState(_))),
        "{refused:?}"
    );
}

/// Locks live on the server. Without one, git-lfs's own words reach the user.
#[test]
fn lock_without_a_server_fails_with_git_lfs_s_output() {
    if !lfs_installed() {
        return;
    }
    let f = test_fixtures::linear(1).unwrap();

    let refused = open(&f).lfs_op(&LfsOp::Lock {
        paths: vec!["file0.txt".to_owned()],
    });

    assert!(matches!(refused, Err(GitError::Command(_))), "{refused:?}");
}

#[test]
fn prune_runs_in_an_lfs_repository() {
    if !lfs_installed() {
        return;
    }
    let f = test_fixtures::linear(1).unwrap();
    open(&f).lfs_op(&LfsOp::Install).unwrap();

    open(&f).lfs_op(&LfsOp::Prune).unwrap();
}
