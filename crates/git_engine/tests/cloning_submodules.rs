//! Include submodules. A submodule on the same disk is a local path, which git refuses by
//! default (R-442); only this binary's global config lifts it.
#![allow(unsafe_code, clippy::unwrap_used)]

use git_engine::{
    CloneRequest, CommandSink, Expected, GitError, GitOutput, NetworkStop, clone_repository,
};

type GitResult<T> = Result<T, GitError>;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn allow_local_submodules(config: &Path) {
    let global = config.join("gitconfig");
    std::fs::write(&global, "[protocol \"file\"]\n\tallow = always\n").unwrap();
    // SAFETY: nextest runs each test in its own process, and this is its only thread then.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    }
}

fn request(source: &Path, target: &Path) -> CloneRequest {
    CloneRequest {
        source: source.to_string_lossy().into_owned(),
        target: target.to_string_lossy().into_owned(),
        submodules: true,
        all_branches: true,
        branch: None,
        skip_larger_than: None,
    }
}

fn clone_journaled(request: &CloneRequest) -> (GitResult<PathBuf>, Vec<GitOutput>) {
    let journal: Arc<Mutex<Vec<GitOutput>>> = Arc::default();
    let sink: CommandSink = {
        let journal = Arc::clone(&journal);
        Arc::new(move |entry| journal.lock().unwrap().push(entry))
    };
    let cloned = clone_repository(request, None, &NetworkStop::default(), Some(&sink), |_| {});
    let entries = journal.lock().unwrap().clone();
    (cloned, entries)
}

/// The repository is cloned and checked out, a submodule is not: the clone is kept and its
/// journal entry is a warning with git's whole output (R-870).
#[test]
fn a_clone_whose_submodule_fails_is_kept_with_a_warning() {
    let config = tempfile::tempdir().unwrap();
    allow_local_submodules(config.path());
    let f = test_fixtures::with_submodule().unwrap();
    let nowhere = config
        .path()
        .join("nowhere")
        .to_string_lossy()
        .replace('\\', "/");
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.vendor/lib.url",
        &nowhere,
    ])
    .unwrap();
    f.git(&["add", ".gitmodules"]).unwrap();
    f.commit_staged(2, "point vendor/lib nowhere").unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");

    let (cloned, journal) = clone_journaled(&request(f.path(), &target));

    assert_eq!(cloned.unwrap(), target);
    assert!(target.join("README.md").is_file(), "checked out");
    let entry = journal.last().unwrap();
    assert_eq!(entry.expected, Some(Expected::SubmodulesFailed));
    assert_ne!(entry.exit_code, Some(0));
    assert!(entry.stderr.contains("nowhere"), "{}", entry.stderr);
}

#[test]
fn a_clone_that_fails_itself_stays_an_error() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");

    let (cloned, journal) = clone_journaled(&request(&parent.path().join("nowhere"), &target));

    assert!(cloned.is_err());
    assert!(!target.exists());
    assert_eq!(journal.last().unwrap().expected, None);
}

#[test]
fn a_clone_with_submodules_checks_them_out() {
    let config = tempfile::tempdir().unwrap();
    allow_local_submodules(config.path());
    let f = test_fixtures::with_submodule().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");

    clone_repository(
        &request(f.path(), &target),
        None,
        &NetworkStop::default(),
        None,
        |_| {},
    )
    .unwrap();

    let out = test_fixtures::git_command_in(&target)
        .args(["submodule", "status"])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&out.stdout);
    assert!(
        status.starts_with(' '),
        "initialized and checked out: {status}"
    );
    assert!(
        std::fs::read_dir(target.join("vendor/lib"))
            .unwrap()
            .count()
            > 1,
        "the submodule has its files"
    );
}
