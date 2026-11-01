#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{
    FileStatus, GcPrune, GitError, HealthIssue, MaintenanceTask, RepoHandle, WorktreeView,
};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn issues(repo: &RepoHandle) -> Vec<HealthIssue> {
    repo.health_report()
        .into_iter()
        .filter(|finding| finding.module.is_empty())
        .map(|finding| finding.issue)
        .collect()
}

#[test]
fn a_file_outside_the_sparse_cone_is_sparse_not_deleted() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("keep/a.txt", "a\n").unwrap();
    f.write_file("drop/b.txt", "b\n").unwrap();
    f.git(&["add", "."]).unwrap();
    f.commit_staged(5, "two folders").unwrap();
    f.git(&["sparse-checkout", "set", "keep"]).unwrap();
    let repo = open(&f);

    let files = repo
        .worktree_files_with(WorktreeView {
            skipped: true,
            ..WorktreeView::default()
        })
        .unwrap();

    let status = |path: &str| {
        files
            .unstaged
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.status)
    };
    assert_eq!(status("drop/b.txt"), Some(FileStatus::Sparse));
    assert!(
        issues(&repo)
            .iter()
            .any(|issue| matches!(issue, HealthIssue::SparseCheckout { .. }))
    );
}

#[test]
fn an_lfs_pointer_on_disk_is_reported() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(
        ".gitattributes",
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    f.write_file(
        "big.bin",
        "version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n",
    )
    .unwrap();
    f.git(&["-c", "filter.lfs.required=false", "add", "."])
        .unwrap();
    f.commit_staged(5, "pointer").unwrap();

    let found = issues(&open(&f));

    assert!(found.iter().any(|issue| matches!(
        issue,
        HealthIssue::LfsPointers { count: 1, sample, .. } if sample == &["big.bin".to_owned()]
    )));
}

#[test]
fn exclude_ignores_locally_and_check_ignore_names_the_rule() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("scratch.log", "x\n").unwrap();
    let repo = open(&f);

    repo.add_to_exclude(&["scratch.log".to_owned()]).unwrap();
    let rules = repo
        .ignore_rules(&["scratch.log".to_owned(), "file0.txt".to_owned()])
        .unwrap();

    assert!(!f.path().join(".gitignore").exists());
    assert!(
        rules[0]
            .source
            .as_deref()
            .is_some_and(|source| source.ends_with("info/exclude"))
    );
    assert_eq!(rules[0].pattern.as_deref(), Some("/scratch.log"));
    assert!(rules[1].source.is_none());
}

#[test]
fn commit_details_carry_trailers_and_the_encoding_header() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"])
        .unwrap();
    f.write_file("new.txt", "n\n").unwrap();
    f.git(&["add", "."]).unwrap();
    let message = f.path().join("msg.txt");
    std::fs::write(
        &message,
        b"caf\xe9\n\nbody\n\nCo-authored-by: Ann <ann@example.com>\n",
    )
    .unwrap();
    f.git(&["commit", "-q", "-F", message.to_str().unwrap()])
        .unwrap();

    let details = open(&f).commit_details("HEAD").unwrap();

    assert_eq!(details.summary, "caf\u{e9}");
    assert!(details.encoding.is_some());
    assert_eq!(details.trailers.len(), 1);
    assert_eq!(details.trailers[0].key, "Co-authored-by");
    assert!(!details.signed);
}

#[test]
fn a_repository_setting_names_the_file_it_comes_from() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "pull.rebase", "true"]).unwrap();

    let settings = open(&f).repo_settings();
    let rebase = settings
        .iter()
        .find(|setting| setting.key == "pull.rebase")
        .unwrap();

    assert_eq!(rebase.scope.as_deref(), Some("local"));
    assert!(
        rebase
            .origin
            .as_deref()
            .is_some_and(|origin| origin.ends_with("config"))
    );
}

#[test]
fn rerere_reports_nothing_while_it_is_off() {
    let f = test_fixtures::conflicted().unwrap();

    let status = open(&f).rerere_status().unwrap();

    assert!(!status.enabled);
    assert!(status.resolved.is_empty());
}

#[test]
fn range_diff_pairs_a_commit_with_its_amended_self() {
    let f = test_fixtures::linear(2).unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["commit", "-q", "--amend", "-m", "reworded"])
        .unwrap();

    let out = open(&f).range_diff(&before, "HEAD").unwrap();

    assert!(out.stdout.contains("reworded"), "{}", out.stdout);
}

fn lock_at(f: &test_fixtures::Fixture, relative: &str, minutes_ago: u64) -> std::path::PathBuf {
    let path = f.path().join(".git/objects/info").join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(minutes_ago * 60);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(then))
        .unwrap();
    path
}

fn stale_graph_lock(repo: &RepoHandle) -> Option<u64> {
    issues(repo).into_iter().find_map(|issue| match issue {
        HealthIssue::StaleCommitGraphLock { minutes } => Some(minutes),
        _ => None,
    })
}

#[test]
fn an_old_commit_graph_lock_is_reported() {
    let f = test_fixtures::linear(1).unwrap();
    lock_at(&f, "commit-graph.lock", 30);
    assert!(stale_graph_lock(&open(&f)).is_some_and(|minutes| minutes >= 29));
}

#[test]
fn an_old_commit_graph_chain_lock_is_reported() {
    let f = test_fixtures::linear(1).unwrap();
    lock_at(&f, "commit-graphs/commit-graph-chain.lock", 30);
    assert!(stale_graph_lock(&open(&f)).is_some());
}

#[test]
fn a_fresh_commit_graph_lock_is_left_to_the_writer() {
    let f = test_fixtures::linear(1).unwrap();
    let lock = lock_at(&f, "commit-graph.lock", 1);
    let repo = open(&f);
    assert_eq!(stale_graph_lock(&repo), None);
    assert!(
        repo.run_maintenance(MaintenanceTask::ClearCommitGraphLock, GcPrune::default())
            .is_err()
    );
    assert!(lock.exists());
}

#[test]
fn clearing_a_stale_commit_graph_lock_removes_it() {
    let f = test_fixtures::linear(1).unwrap();
    let lock = lock_at(&f, "commit-graph.lock", 30);
    let chain = lock_at(&f, "commit-graphs/commit-graph-chain.lock", 30);
    let repo = open(&f);
    repo.run_maintenance(MaintenanceTask::ClearCommitGraphLock, GcPrune::default())
        .unwrap();
    assert!(!lock.exists());
    assert!(!chain.exists());
    assert_eq!(stale_graph_lock(&repo), None);
}

#[test]
fn writing_the_commit_graph_under_a_lock_names_the_lock() {
    let f = test_fixtures::linear(1).unwrap();
    lock_at(&f, "commit-graph.lock", 1);
    let repo = open(&f);
    for task in [MaintenanceTask::CommitGraph, MaintenanceTask::Gc] {
        let err = repo.run_maintenance(task, GcPrune::default()).unwrap_err();
        assert!(
            matches!(err, GitError::InvalidState(ref text) if text.contains("commit-graph.lock")),
            "{err:?}"
        );
    }
}
