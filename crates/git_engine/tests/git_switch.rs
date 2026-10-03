// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Preferences ▸ Git executable changed while Cogit runs. The git in use is process-wide,
//! so this has a test binary of its own.

use std::time::Duration;

#[test]
fn a_working_git_is_switched_to_and_a_broken_one_leaves_the_git_in_use() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-git.exe");
    let before = git_engine::git_version().unwrap();

    let refused =
        git_engine::switch_git(&missing.display().to_string(), Duration::from_secs(5)).unwrap_err();
    assert!(
        matches!(&refused, git_engine::GitError::GitNotFound(said) if said.contains(&missing.display().to_string())),
        "{refused:?}"
    );
    assert_eq!(git_engine::git_version().unwrap(), before);

    let probe = git_engine::switch_git("  ", Duration::from_secs(5)).unwrap();
    assert!(probe.valid);
    assert_eq!(git_engine::git_version().unwrap(), before);
}
