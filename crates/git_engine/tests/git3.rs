#![allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]

use git_engine::{
    CloneRequest, FolderKind, GitError, NetworkStop, RepoHandle, SharedRepo, clone_repository,
    folder_kind, init_repository,
};
use std::path::{Path, PathBuf};

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = test_fixtures::git_command_in(dir)
        .args(args)
        .env("GIT_AUTHOR_DATE", "1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "1700000000 +0000")
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn repository(formats: &[&str]) -> Option<tempfile::TempDir> {
    let dir = tempfile::tempdir().unwrap();
    let mut init = vec!["init", "-q", "-b", "main"];
    init.extend_from_slice(formats);
    git(dir.path(), &init)?;
    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    git(dir.path(), &["add", "a.txt"]).unwrap();
    git(dir.path(), &["commit", "-q", "-m", "one"]).unwrap();
    Some(dir)
}

fn refusal(result: Result<impl std::fmt::Debug, GitError>) -> String {
    match result {
        Err(GitError::Unavailable(text)) => text,
        other => panic!("expected Cogit's own refusal, got {other:?}"),
    }
}

fn own_global(dir: &Path, text: &str) {
    let global = dir.join("gitconfig");
    std::fs::write(&global, text).unwrap();
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    }
}

#[test]
fn a_sha256_repository_is_refused_by_name_not_reported_missing() {
    let Some(dir) = repository(&["--object-format=sha256", "--ref-format=files"]) else {
        return;
    };

    let text = refusal(RepoHandle::open(dir.path()));
    assert!(text.contains("SHA-256"), "{text}");
    let text = refusal(SharedRepo::open(dir.path()));
    assert!(text.contains("SHA-256"), "{text}");
    assert_eq!(folder_kind(dir.path()), FolderKind::Repository);
}

#[test]
fn a_reftable_repository_is_refused_with_the_way_out() {
    let Some(dir) = repository(&["--object-format=sha1", "--ref-format=reftable"]) else {
        return;
    };

    let text = refusal(RepoHandle::open(dir.path()));
    assert!(text.contains("reftable"), "{text}");
    assert!(
        text.contains("git refs migrate --ref-format=files"),
        "{text}"
    );
    let text = refusal(SharedRepo::open(dir.path()));
    assert!(text.contains("reftable"), "{text}");
    assert_eq!(folder_kind(dir.path()), FolderKind::Repository);
}

#[test]
fn a_reftable_repository_migrated_to_files_opens() {
    let Some(dir) = repository(&["--object-format=sha1", "--ref-format=reftable"]) else {
        return;
    };
    if git(dir.path(), &["refs", "migrate", "--ref-format=files"]).is_none() {
        return;
    }

    let repo = RepoHandle::open(dir.path()).unwrap();
    let names: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert_eq!(names, ["main"]);
}

const GIT3_DEFAULTS: &str = "[init]\n\
    \tdefaultObjectFormat = sha256\n\
    \tdefaultRefFormat = reftable\n\
    [safe]\n\
    \tbareRepository = explicit\n";

#[test]
fn init_makes_a_repository_cogit_can_read_whatever_the_defaults() {
    let home = tempfile::tempdir().unwrap();
    own_global(home.path(), GIT3_DEFAULTS);
    let folder = home.path().join("fresh");

    init_repository(&folder, None).unwrap();

    RepoHandle::open_root(&folder).unwrap();
}

#[test]
fn a_clone_is_a_repository_cogit_can_read_whatever_the_defaults() {
    let home = tempfile::tempdir().unwrap();
    own_global(home.path(), GIT3_DEFAULTS);
    let server = test_fixtures::linear(2).unwrap();
    let target: PathBuf = home.path().join("copy");
    let request = CloneRequest {
        source: server.path().to_string_lossy().into_owned(),
        target: target.to_string_lossy().into_owned(),
        submodules: false,
        all_branches: true,
        branch: None,
        depth: None,
        skip_larger_than: None,
    };

    clone_repository(&request, None, &NetworkStop::default(), None, |_| {}).unwrap();

    let repo = RepoHandle::open_root(&target).unwrap();
    assert!(!repo.branches().unwrap().is_empty());
}

#[test]
fn a_bare_repository_still_runs_git_when_bare_ones_must_be_explicit() {
    let home = tempfile::tempdir().unwrap();
    own_global(home.path(), GIT3_DEFAULTS);
    let f = test_fixtures::bare().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let out = repo
        .run_git(&["rev-parse", "--is-bare-repository"])
        .unwrap();

    assert_eq!(out.stdout.trim(), "true");
}
