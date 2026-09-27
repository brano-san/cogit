#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileStatus, HealthIssue, RepoHandle, WorktreeView};

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
