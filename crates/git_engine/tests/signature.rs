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

#[test]
fn a_batch_reads_each_commit_status_by_id() {
    let Some(f) = signed() else {
        return;
    };
    let repo = RepoHandle::open(f.path()).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).unwrap().trim().to_owned();
    let parent = f.git(&["rev-parse", "HEAD~1"]).unwrap().trim().to_owned();

    let found = repo
        .commit_signatures(&[parent.clone(), head.clone()])
        .unwrap();

    let status = |oid: &str| {
        found
            .iter()
            .find(|entry| entry.oid == oid)
            .map(|entry| entry.status.clone())
    };
    assert_eq!(status(&parent).as_deref(), Some("N"), "{found:?}");
    assert_eq!(status(&head).as_deref(), Some("E"), "{found:?}");
}

#[test]
fn a_batch_skips_commits_this_repository_does_not_have() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).unwrap().trim().to_owned();
    let foreign = "292aa3e041f9aaf23ac0b41133da60d6371f5af0".to_owned();

    let found = repo.commit_signatures(&[foreign, head.clone()]).unwrap();

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].oid, head);
    assert!(
        repo.commit_signatures(&["not-an-id".to_owned()])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_batch_verifies_against_the_allowed_signers() {
    let Some(f) = signed() else {
        return;
    };
    let public = std::fs::read_to_string(f.path().join(".signing-key.pub")).unwrap();
    let allowed = f.path().join(".allowed-signers");
    std::fs::write(&allowed, format!("* {public}")).unwrap();
    let allowed = allowed.to_string_lossy().replace('\\', "/");
    f.git(&["config", "gpg.ssh.allowedSignersFile", &allowed])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).unwrap().trim().to_owned();

    let found = repo.commit_signatures(&[head]).unwrap();

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].status, "G", "{found:?}");
}
