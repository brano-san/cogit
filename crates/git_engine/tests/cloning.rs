//! Repository ▸ Clone… against a local "remote": the check, the clone and its options,
//! cancelling (F-575).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{
    CloneDestination, CloneRequest, GitError, NetworkStop, clone_destination, clone_repository,
    remote_branches, repository_url_in,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// main and dev from the fixture, and a branch in a folder.
fn server() -> test_fixtures::Fixture {
    let f = test_fixtures::branched().unwrap();
    f.git(&["branch", "feature/x", "dev"]).unwrap();
    f
}

fn source_of(f: &test_fixtures::Fixture) -> String {
    f.path().to_string_lossy().into_owned()
}

fn request(source: &str, target: &Path) -> CloneRequest {
    CloneRequest {
        source: source.to_owned(),
        target: target.to_string_lossy().into_owned(),
        submodules: true,
        all_branches: true,
        branch: None,
        skip_larger_than_mb: None,
    }
}

fn clone(request: &CloneRequest) -> Result<PathBuf, GitError> {
    clone_repository(request, None, &NetworkStop::default(), None, |_| {})
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = test_fixtures::git_command_in(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn tracking_refs(clone: &Path) -> Vec<String> {
    git_in(
        clone,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin",
        ],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

#[test]
fn the_check_lists_every_branch_and_the_one_head_names() {
    let f = server();

    let found = remote_branches(&source_of(&f), None, None).unwrap();

    assert_eq!(found.default_branch.as_deref(), Some("main"));
    let heads: Vec<String> = git_in(f.path(), &["ls-remote", "--heads", &source_of(&f)])
        .lines()
        .filter_map(|line| {
            line.split_once("refs/heads/")
                .map(|(_, name)| name.to_owned())
        })
        .collect();
    assert_eq!(found.branches, heads, "as git ls-remote --heads lists them");
    assert!(found.branches.contains(&"feature/x".to_owned()));
}

#[test]
fn an_empty_repository_has_a_head_and_no_branch() {
    let f = test_fixtures::empty().unwrap();

    let found = remote_branches(&source_of(&f), None, None).unwrap();

    assert!(found.branches.is_empty(), "{found:?}");
}

#[test]
fn a_check_that_fails_says_what_git_said_and_is_journalled() {
    let missing = tempfile::tempdir().unwrap().path().join("nothing-here");
    let journal: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink: git_engine::CommandSink = {
        let journal = Arc::clone(&journal);
        Arc::new(move |out: git_engine::GitOutput| journal.lock().unwrap().push(out.command))
    };

    let err = remote_branches(&missing.to_string_lossy(), None, Some(&sink)).unwrap_err();

    let GitError::Command(failure) = err else {
        panic!("a refusal from git is a command error: {err:?}");
    };
    assert!(!failure.stderr.trim().is_empty(), "{failure:?}");
    assert!(failure.command.contains("ls-remote"), "{}", failure.command);
    assert_eq!(journal.lock().unwrap().len(), 1);
}

#[test]
fn a_clone_checks_out_the_default_branch_and_fetches_every_other() {
    let f = server();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");

    let root = clone(&request(&source_of(&f), &target)).unwrap();

    assert_eq!(root, target);
    assert_eq!(git_in(&target, &["branch", "--show-current"]), "main");
    let refs = tracking_refs(&target);
    for branch in ["origin/main", "origin/dev", "origin/feature/x"] {
        assert!(refs.iter().any(|name| name == branch), "{branch}: {refs:?}");
    }
}

#[test]
fn a_clone_of_one_branch_checks_it_out_and_fetches_no_other() {
    let f = server();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    let only_dev = CloneRequest {
        all_branches: false,
        branch: Some("dev".to_owned()),
        ..request(&source_of(&f), &target)
    };

    clone(&only_dev).unwrap();

    assert_eq!(git_in(&target, &["branch", "--show-current"]), "dev");
    assert_eq!(tracking_refs(&target), ["origin/dev"]);
}

#[test]
fn a_clone_without_submodules_leaves_them_uninitialized() {
    let f = test_fixtures::with_submodule().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    let without = CloneRequest {
        submodules: false,
        ..request(&source_of(&f), &target)
    };

    clone(&without).unwrap();

    let status = git_in(&target, &["submodule", "status"]);
    assert!(status.starts_with('-'), "not initialized: {status}");
}

/// The checkout fetches what it needs, so only a large file that HEAD no longer has stays
/// on the server.
#[test]
fn skipping_large_files_makes_a_partial_clone() {
    let f = server();
    f.commit_file(10, "big.bin", &"x".repeat(2 * 1024 * 1024))
        .unwrap();
    f.git(&["rm", "-q", "big.bin"]).unwrap();
    f.commit_staged(11, "drop big.bin").unwrap();
    f.git(&["config", "uploadpack.allowFilter", "true"])
        .unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    // A plain path is a local clone, which ignores `--filter`; `file://` goes through
    // upload-pack as a server would.
    let url = format!(
        "file:///{}",
        source_of(&f).replace('\\', "/").trim_start_matches('/')
    );
    let partial = CloneRequest {
        skip_larger_than_mb: Some(1),
        ..request(&url, &target)
    };

    clone(&partial).unwrap();

    assert_eq!(
        git_in(&target, &["config", "remote.origin.promisor"]),
        "true"
    );
    assert_eq!(
        git_in(&target, &["config", "remote.origin.partialclonefilter"]),
        "blob:limit=1048576"
    );
    let objects = git_in(
        &target,
        &["rev-list", "--objects", "--all", "--missing=print"],
    );
    assert_eq!(
        objects.lines().filter(|line| line.starts_with('?')).count(),
        1,
        "only big.bin is left on the server"
    );
}

#[test]
fn a_folder_that_is_not_empty_is_refused_by_git_and_left_alone() {
    let f = server();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("mine.txt"), "mine\n").unwrap();

    let err = clone(&request(&source_of(&f), &target)).unwrap_err();

    let GitError::Command(failure) = err else {
        panic!("git refuses it: {err:?}");
    };
    assert!(
        failure.stderr.contains("not an empty directory"),
        "{failure:?}"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("mine.txt")).unwrap(),
        "mine\n"
    );
}

/// A server that takes the connection and never says a word.
fn silent_server() -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let held: Vec<_> = server.incoming().take(4).collect();
        std::thread::sleep(Duration::from_secs(120));
        drop(held);
    });
    format!("git://127.0.0.1:{port}/x.git")
}

/// Cancels once git has made the folder, and gives back what the clone returned.
fn cancelled_clone(target: &Path) -> Result<PathBuf, GitError> {
    let stop = NetworkStop::default();
    let (done, finished) = std::sync::mpsc::channel();
    {
        let (stop, request) = (stop.clone(), request(&silent_server(), target));
        std::thread::spawn(move || {
            let _ = done.send(clone_repository(&request, None, &stop, None, |_| {}));
        });
    }
    let started = Instant::now();
    while !target.join(".git").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "git never made the folder"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(stop.stop());
    finished
        .recv_timeout(Duration::from_secs(20))
        .expect("the clone was still running 20 s after it was cancelled")
}

#[test]
fn a_cancelled_clone_ends_as_cancelled_and_leaves_no_folder() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");

    let result = cancelled_clone(&target);

    assert!(matches!(result, Err(GitError::Cancelled(_))), "{result:?}");
    assert!(!target.exists(), "the half-made clone is gone");
}

#[test]
fn a_cancelled_clone_into_an_empty_folder_keeps_the_folder_empty() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    std::fs::create_dir(&target).unwrap();

    let result = cancelled_clone(&target);

    assert!(matches!(result, Err(GitError::Cancelled(_))), "{result:?}");
    assert_eq!(clone_destination(&target), CloneDestination::Empty);
}

#[test]
fn the_destination_says_what_is_there() {
    let parent = tempfile::tempdir().unwrap();
    let folder = parent.path().join("folder");
    assert_eq!(clone_destination(&folder), CloneDestination::Missing);
    std::fs::create_dir(&folder).unwrap();
    assert_eq!(clone_destination(&folder), CloneDestination::Empty);
    std::fs::write(folder.join("a.txt"), "a").unwrap();
    assert_eq!(clone_destination(&folder), CloneDestination::NotEmpty);
    assert_eq!(
        clone_destination(&folder.join("a.txt")),
        CloneDestination::File
    );
}

#[test]
fn a_repository_url_in_the_clipboard_is_recognized() {
    for url in [
        "https://github.com/tauri-apps/tauri",
        "https://github.com/tauri-apps/tauri.git",
        "https://gitlab.example.com/group/sub/app.git",
        "https://dev.azure.com/org/project/_git/app",
        "git@github.com:owner/app.git",
        "ssh://git@example.com:2222/srv/app.git",
        "git://example.com/app",
        "file:///D:/repos/app",
        "  https://codeberg.org/owner/app/  \n",
    ] {
        assert_eq!(repository_url_in(url).as_deref(), Some(url.trim()), "{url}");
    }
}

#[test]
fn other_text_in_the_clipboard_is_not_taken_for_a_url() {
    for text in [
        "",
        "hello world",
        "https://example.com/some/article",
        "https://github.com/owner/app/blob/main/README.md",
        "https://github.com/owner/app?tab=readme",
        "C:\\work\\app",
        "D:/work/app",
        "--upload-pack=touch x",
        "git clone https://github.com/owner/app.git",
        "mailto:someone@example.com",
    ] {
        assert_eq!(repository_url_in(text), None, "{text}");
    }
}
