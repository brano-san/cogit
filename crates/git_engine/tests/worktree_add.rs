#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Add Worktree: a new branch from any start point, tracking, detached, an existing branch,
//! and the probes behind the dialog's inline checks (item 13).

use git_engine::{RepoHandle, WorktreeBranch};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn folder(aux: &tempfile::TempDir, name: &str) -> String {
    aux.path().join(name).to_string_lossy().replace("\\", "/")
}

fn entry(f: &test_fixtures::Fixture, path: &str) -> git_engine::WorktreeEntry {
    open(f)
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|entry| entry.path == path)
        .unwrap_or_else(|| panic!("{path} is not listed"))
}

#[test]
fn a_remote_start_point_makes_a_tracking_branch() {
    let f = test_fixtures::with_remote().unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "feat-wt");

    open(&f)
        .add_worktree_with(
            &path,
            &WorktreeBranch::New {
                name: "feat".into(),
                start: Some("origin/main".into()),
                track: true,
            },
        )
        .unwrap();

    assert_eq!(entry(&f, &path).branch.as_deref(), Some("feat"));
    assert_eq!(entry(&f, &path).head, f.oid("origin/main").unwrap());
    assert_eq!(
        f.git(&["config", "branch.feat.remote"]).unwrap().trim(),
        "origin"
    );
}

#[test]
fn a_new_branch_without_tracking_has_no_upstream() {
    let f = test_fixtures::with_remote().unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "plain-wt");

    open(&f)
        .add_worktree_with(
            &path,
            &WorktreeBranch::New {
                name: "plain".into(),
                start: Some("origin/main".into()),
                track: false,
            },
        )
        .unwrap();

    assert!(f.git(&["config", "branch.plain.remote"]).is_err());
}

#[test]
fn detached_puts_head_on_the_commit_without_a_branch() {
    let f = test_fixtures::linear(3).unwrap();
    let first = f.oid("HEAD~2").unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "detached-wt");

    open(&f)
        .add_worktree_with(
            &path,
            &WorktreeBranch::Detached {
                start: Some(first.clone()),
            },
        )
        .unwrap();

    let added = entry(&f, &path);
    assert_eq!(added.branch, None);
    assert_eq!(added.head, first);
}

#[test]
fn an_existing_branch_is_checked_out_as_it_is() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["branch", "spare"]).unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "spare-wt");

    open(&f)
        .add_worktree_with(
            &path,
            &WorktreeBranch::Existing {
                name: "spare".into(),
            },
        )
        .unwrap();

    assert_eq!(entry(&f, &path).branch.as_deref(), Some("spare"));
}

#[test]
fn a_taken_branch_name_is_refused_with_gits_own_words() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["branch", "taken"]).unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "clash-wt");

    let err = open(&f)
        .add_worktree_with(
            &path,
            &WorktreeBranch::New {
                name: "taken".into(),
                start: None,
                track: false,
            },
        )
        .unwrap_err();

    assert!(err.to_string().contains("worktree"), "{err}");
    assert!(!std::path::Path::new(&path).exists());
}

#[test]
fn a_folder_with_files_in_it_is_refused() {
    let f = test_fixtures::linear(2).unwrap();
    let aux = test_fixtures::tempdir().unwrap();
    let path = folder(&aux, "busy");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(std::path::Path::new(&path).join("keep.txt"), "mine").unwrap();

    let result = open(&f).add_worktree_with(&path, &WorktreeBranch::Detached { start: None });

    assert!(result.is_err());
    assert!(std::path::Path::new(&path).join("keep.txt").exists());
}

#[test]
fn a_revision_resolves_to_its_commit_with_a_preview() {
    let f = test_fixtures::linear(3).unwrap();
    let want = f.oid("HEAD~1").unwrap();

    let check = open(&f).check_revision("HEAD~1").unwrap();

    let commit = check.commit.expect("HEAD~1 is a commit");
    assert_eq!(commit.oid, want);
    assert!(commit.oid.starts_with(&commit.short_oid));
    assert!(!commit.subject.is_empty());
    assert!(commit.date > 0);
    assert_eq!(check.problem, None);
}

#[test]
fn a_short_hash_and_a_tag_resolve_too() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "v1"]).unwrap();
    let head = f.oid("HEAD").unwrap();

    assert_eq!(
        open(&f)
            .check_revision(&head[..8])
            .unwrap()
            .commit
            .unwrap()
            .oid,
        head
    );
    assert_eq!(
        open(&f).check_revision("v1").unwrap().commit.unwrap().oid,
        head
    );
}

#[test]
fn an_unknown_revision_is_a_problem_not_an_error() {
    let f = test_fixtures::linear(2).unwrap();

    let check = open(&f).check_revision("no-such-thing").unwrap();

    assert!(check.commit.is_none());
    assert!(check.problem.unwrap().contains("no-such-thing"));
}

#[test]
fn a_revision_cannot_be_read_as_an_option() {
    let f = test_fixtures::linear(2).unwrap();

    let check = open(&f).check_revision("--all").unwrap();

    assert!(check.commit.is_none());
    assert!(check.problem.is_some());
}

#[test]
fn a_probe_leaves_no_journal_entry() {
    let f = test_fixtures::linear(2).unwrap();
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter = seen.clone();
    let handle = open(&f).with_journal(std::sync::Arc::new(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    handle.check_revision("nope").unwrap();
    handle.check_branch_name("a..b").unwrap();

    assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn branch_names_are_checked_by_git() {
    let f = test_fixtures::linear(1).unwrap();
    let handle = open(&f);

    assert_eq!(handle.check_branch_name("feature/x").unwrap(), None);
    assert!(handle.check_branch_name("a..b").unwrap().is_some());
    assert!(handle.check_branch_name("has space").unwrap().is_some());
    assert!(handle.check_branch_name("-dash").unwrap().is_some());
    assert!(handle.check_branch_name("").unwrap().is_some());
}

#[test]
fn a_folder_that_is_missing_or_empty_can_take_a_worktree() {
    let aux = test_fixtures::tempdir().unwrap();
    let empty = aux.path().join("empty");
    std::fs::create_dir(&empty).unwrap();

    assert_eq!(git_engine::worktree_folder_problem(&empty), None);
    assert_eq!(
        git_engine::worktree_folder_problem(&aux.path().join("not-yet")),
        None
    );
}

#[test]
fn a_folder_with_files_or_a_file_cannot() {
    let aux = test_fixtures::tempdir().unwrap();
    let busy = aux.path().join("busy");
    std::fs::create_dir(&busy).unwrap();
    std::fs::write(busy.join("a.txt"), "x").unwrap();
    let file = aux.path().join("file");
    std::fs::write(&file, "x").unwrap();

    assert!(
        git_engine::worktree_folder_problem(&busy)
            .unwrap()
            .contains("not empty")
    );
    assert!(
        git_engine::worktree_folder_problem(&file)
            .unwrap()
            .contains("file")
    );
}
