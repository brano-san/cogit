// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `log.showSignature=true` in the user's config prints the verification above every
//! `--format` the app parses: the signature status read "No signature\nN".

use git_engine::RepoHandle;
use std::process::Command;

/// A commit signed with a throwaway SSH key; `None` where `ssh-keygen` cannot be run.
fn signed() -> Option<test_fixtures::Fixture> {
    let f = test_fixtures::linear(1).unwrap();
    let key = f.path().join(".signing-key");
    let made = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
        .arg(&key)
        .output()
        .ok()?;
    if !made.status.success() {
        return None;
    }
    let key = key.to_string_lossy().replace('\\', "/");
    f.git(&["config", "gpg.format", "ssh"]).unwrap();
    f.git(&["config", "user.signingkey", &key]).unwrap();
    f.write_file("signed.txt", "s\n").unwrap();
    f.git(&["add", "--", "signed.txt"]).unwrap();
    f.git(&["commit", "-q", "-S", "-m", "signed"]).unwrap();
    f.git(&["config", "log.showSignature", "true"]).unwrap();
    Some(f)
}

#[test]
fn the_signature_status_is_read_with_show_signature_on() {
    let Some(f) = signed() else {
        return;
    };
    let repo = RepoHandle::open(f.path()).unwrap();

    let check = repo.commit_signature("HEAD").unwrap();

    assert_eq!(check.status.len(), 1, "{check:?}");
}
