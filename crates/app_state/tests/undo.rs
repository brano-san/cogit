// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn nothing_to_undo_on_a_fresh_repository() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);

    assert!(state.safety_log().is_empty());
    assert!(state.undo_last(repo).is_err());
}

#[test]
fn discarding_a_file_is_undoable_and_restores_the_exact_content() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("file0.txt"), "work in progress\n").unwrap();

    state
        .discard_paths(repo, &["file0.txt".to_owned()])
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "content 0\n"
    );

    state.undo_last(repo).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "work in progress\n"
    );
}

#[test]
fn discarding_an_untracked_file_is_undoable() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("scratch.txt"), "not added yet\n").unwrap();

    state
        .discard_paths(repo, &["scratch.txt".to_owned()])
        .unwrap();
    assert!(!f.path().join("scratch.txt").exists());

    state.undo_last(repo).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("scratch.txt")).unwrap(),
        "not added yet\n"
    );
}

#[test]
fn deleting_a_branch_is_undoable_and_restores_the_same_oid() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    let before = f.oid("dev").unwrap();

    state.delete_branch(repo, "dev", true).unwrap();
    assert!(f.oid("dev").is_err());

    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("dev").unwrap(), before);
}

#[test]
fn the_journal_describes_what_happened() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);

    state.delete_branch(repo, "dev", true).unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    assert!(entry.description.contains("dev"), "{entry:?}");
    assert!(entry.undoable);
}

#[test]
fn an_operation_that_cannot_be_undone_says_so() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);

    state
        .checkout(
            repo,
            &git_engine::CheckoutTarget::Branch {
                name: "main".to_owned(),
            },
        )
        .unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    assert!(
        !entry.undoable,
        "an honest journal marks what it cannot reverse: {entry:?}"
    );
}

#[test]
fn undo_refuses_an_entry_it_cannot_reverse() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    state
        .checkout(
            repo,
            &git_engine::CheckoutTarget::Branch {
                name: "main".to_owned(),
            },
        )
        .unwrap();

    assert!(state.undo_last(repo).is_err());
}

#[test]
fn an_undone_entry_is_not_offered_twice() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    state.delete_branch(repo, "dev", true).unwrap();

    state.undo_last(repo).unwrap();

    assert!(
        state.undo_last(repo).is_err(),
        "the same deletion must not be undone twice"
    );
}

#[test]
fn the_newest_entry_is_undone_first() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("a.txt"), "first\n").unwrap();
    state.discard_paths(repo, &["a.txt".to_owned()]).unwrap();
    std::fs::write(f.path().join("b.txt"), "second\n").unwrap();
    state.discard_paths(repo, &["b.txt".to_owned()]).unwrap();

    state.undo_last(repo).unwrap();

    assert!(f.path().join("b.txt").exists());
    assert!(
        !f.path().join("a.txt").exists(),
        "only the newest is undone"
    );
}

#[test]
fn discarding_nothing_records_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);

    let _ = state.discard_paths(repo, &[]);

    assert!(state.safety_log().is_empty());
}

#[test]
fn a_mutation_does_not_make_the_watcher_report_our_own_writes() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    let mut events = state.subscribe();
    std::fs::write(f.path().join("file0.txt"), "edited by us\n").unwrap();
    // Let the edit above settle so only the mutation's own writes are in play.
    std::thread::sleep(std::time::Duration::from_millis(500));
    while events.try_recv().is_ok() {}

    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut reported = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let app_state::AppEvent::RepoChanged { kind, .. } = event {
            reported.push(kind);
        }
    }
    assert!(
        reported.is_empty(),
        "the UI reloads itself after a mutation; a watcher event on top makes it flicker, got {reported:?}"
    );
}

#[test]
fn an_older_entry_can_be_undone_out_of_order() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    f.git(&["branch", "first"]).unwrap();
    f.git(&["branch", "second"]).unwrap();
    let oid = f.oid("first").unwrap();

    state.delete_branch(repo, "first", false).unwrap();
    state.delete_branch(repo, "second", false).unwrap();

    // The recoveries are independent restores, not a stack, so order is the user's choice.
    let older = state
        .safety_log()
        .into_iter()
        .find(|entry| entry.description.contains("first"))
        .unwrap();
    state.undo_entry(repo, older.id).unwrap();

    assert_eq!(f.oid("first").unwrap(), oid);
    assert!(f.oid("second").is_err(), "the newer entry must still stand");
}

#[test]
fn undoing_an_entry_removes_it_from_the_journal() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    f.git(&["branch", "gone"]).unwrap();
    state.delete_branch(repo, "gone", false).unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    state.undo_entry(repo, entry.id).unwrap();

    assert!(state.safety_log().iter().all(|kept| kept.id != entry.id));
}

#[test]
fn an_unknown_entry_id_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    assert!(state.undo_entry(repo, 9_999).is_err());
}

#[test]
fn an_entry_belonging_to_another_repository_is_refused() {
    let a = test_fixtures::linear(1).unwrap();
    let b = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let first = state.open_repository(a.path()).unwrap().repo;
    let second = state.open_repository(b.path()).unwrap().repo;

    a.git(&["branch", "doomed"]).unwrap();
    state.delete_branch(first, "doomed", false).unwrap();
    let entry = state.safety_log().into_iter().next().unwrap();

    assert!(state.undo_entry(second, entry.id).is_err());
}

#[test]
fn the_journal_does_not_ship_the_recovery_payload_over_ipc() {
    // A line-level discard keeps the whole patch to undo with. It is the size of the
    // change, it is of no use to the panel, and it crosses the boundary on every read.
    let json = serde_json::to_string(&app_state::SafetyEntry {
        id: 1,
        repo: app_state::RepoId(1),
        description: "Discard lines in a.txt".to_owned(),
        undoable: true,
    })
    .unwrap();

    assert!(!json.contains("recovery"), "{json}");
    assert!(json.contains("undoable"), "{json}");
}

#[test]
fn discarding_a_long_list_of_files_is_undoable() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    // Short enough for one command line, too long to repeat in the stash message as well.
    let paths: Vec<String> = (0..400)
        .map(|i| format!("a-file-whose-name-is-long-enough-to-matter-{i:04}.txt"))
        .collect();
    for path in &paths {
        std::fs::write(f.path().join(path), "not added yet\n").unwrap();
    }

    state.discard_paths(repo, &paths).unwrap();
    assert!(!f.path().join(&paths[0]).exists());

    state.undo_last(repo).unwrap();

    assert!(paths.iter().all(|path| f.path().join(path).exists()));
}

#[test]
fn a_mutation_longer_than_the_quiet_window_does_not_echo_either() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("file0.txt"), "edited by us\n").unwrap();
    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    // Slower than the window: the commit writes HEAD and the index after it would close.
    std::fs::write(
        f.path().join(".git/hooks/pre-commit"),
        "#!/bin/sh\nsleep 1\n",
    )
    .unwrap();
    let mut events = state.subscribe();
    std::thread::sleep(std::time::Duration::from_millis(600));
    while events.try_recv().is_ok() {}

    state
        .commit(
            repo,
            &git_engine::CommitRequest {
                message: "slow hook".to_owned(),
                amend: false,
                no_verify: false,
                only: Vec::new(),
            },
        )
        .unwrap();
    // The window is what the product promises: open through the whole mutation and for
    // DEFAULT_QUIET after its last git process, however long the process took.
    assert!(
        state.watcher_is_quiet(repo),
        "the window must still be open when the mutation returns"
    );
    std::thread::sleep(std::time::Duration::from_millis(150));

    let mut reported = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let app_state::AppEvent::RepoChanged { kind, .. } = event {
            reported.push(kind);
        }
    }
    assert!(
        reported.is_empty(),
        "the commit flow reloads everything itself, got {reported:?}"
    );
}

#[test]
fn a_mutation_that_idles_after_its_last_write_does_not_echo() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(
        f.path().join("file0.txt"),
        "edited by us
",
    )
    .unwrap();
    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    // The refs and the reflog are written before this hook runs; git then idles past the
    // window that opened when the mutation began, with the guard still held.
    std::fs::write(
        f.path().join(".git/hooks/post-commit"),
        "#!/bin/sh
sleep 1
",
    )
    .unwrap();
    let mut events = state.subscribe();
    std::thread::sleep(std::time::Duration::from_millis(600));
    while events.try_recv().is_ok() {}

    state
        .commit(
            repo,
            &git_engine::CommitRequest {
                message: "slow after write".to_owned(),
                amend: false,
                no_verify: false,
                only: Vec::new(),
            },
        )
        .unwrap();
    std::thread::sleep(fs_watcher::DEFAULT_QUIET - std::time::Duration::from_millis(50));

    let mut reported = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let app_state::AppEvent::RepoChanged { kind, .. } = event {
            reported.push(kind);
        }
    }
    assert!(
        reported.is_empty(),
        "the window must follow every git process, not only the start of the mutation, got {reported:?}"
    );
}

#[test]
fn a_hard_reset_keeps_the_changes_it_throws_away_for_undo() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("file0.txt"), "work in progress\n").unwrap();
    let target = f.oid("HEAD~1").unwrap();

    state
        .reset_to(repo, &target, git_engine::ResetMode::Hard)
        .unwrap();
    assert_eq!(f.oid("HEAD").unwrap(), target);
    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "content 0\n"
    );

    state.undo_last(repo).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "work in progress\n"
    );
}

#[test]
fn a_reset_is_journalled_with_where_the_branch_was() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let before = f.oid("HEAD").unwrap();

    state
        .reset_to(
            repo,
            &f.oid("HEAD~2").unwrap(),
            git_engine::ResetMode::Mixed,
        )
        .unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    assert!(
        entry.description.contains(&before[..7]),
        "{}",
        entry.description
    );
    assert!(entry.description.contains("mixed"), "{}", entry.description);
}

fn head_oid(state: &AppState, repo: RepoId) -> String {
    match state
        .open_repository(&state.get(repo).unwrap().root)
        .unwrap()
        .head
    {
        git_engine::Head::Branch { oid, .. } => oid,
        other => panic!("expected a branch, got {other:?}"),
    }
}

// Undo recorded the branch as if it had been deleted and ran `git branch main <old>`,
// which fails with "a branch named 'main' already exists" — every time.
#[test]
fn undoing_a_merge_puts_the_branch_back_where_it_was() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    let before = head_oid(&state, repo);

    state
        .merge(
            repo,
            &git_engine::MergeOptions {
                source: "dev".to_owned(),
                no_fast_forward: true,
                squash: false,
                message: None,
            },
        )
        .unwrap();
    assert_ne!(head_oid(&state, repo), before);

    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), before);
}

#[test]
fn undoing_a_merge_moves_the_branch_back_after_the_user_left_it() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    let before = head_oid(&state, repo);
    state
        .merge(
            repo,
            &git_engine::MergeOptions {
                source: "dev".to_owned(),
                no_fast_forward: true,
                squash: false,
                message: None,
            },
        )
        .unwrap();
    let switched = std::process::Command::new("git")
        .args(["switch", "--quiet", "dev"])
        .current_dir(f.path())
        .status()
        .unwrap();
    assert!(switched.success());

    state.undo_last(repo).unwrap();

    let summary = state.open_repository(f.path()).unwrap();
    let main = summary.branches.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.oid, before);
}

// The dialog says "Undo can bring them back". When the backup stash failed — here because
// a repository without a first commit cannot stash — the files were deleted anyway, with
// nothing to bring back.
#[test]
fn a_discard_whose_backup_fails_throws_nothing_away() {
    let f = test_fixtures::empty().unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("draft.txt"), "only copy\n").unwrap();

    let refused = state.discard_paths(repo, &["draft.txt".to_owned()]);

    assert!(refused.is_err());
    assert_eq!(
        std::fs::read_to_string(f.path().join("draft.txt")).unwrap(),
        "only copy\n"
    );
}

// A moved submodule counts as a change, but `git stash` saves nothing for it (exit 0), and
// the hard reset stopped with "there is nothing to stash" — though `reset --hard` leaves
// the submodule alone and there was nothing to lose. It goes ahead now, without a backup,
// and the user's older stash is not mistaken for one.
#[test]
fn a_hard_reset_with_only_a_moved_submodule_goes_ahead_without_a_backup() {
    let f = test_fixtures::with_submodule().unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("README.md"), "parked earlier\n").unwrap();
    f.git(&["stash", "push", "--message", "the user's own"])
        .unwrap();
    let module = f.path().join("vendor/lib");
    let moved = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=a",
            "-c",
            "user.email=a@b",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "moved",
        ])
        .current_dir(&module)
        .status()
        .unwrap();
    assert!(moved.success());

    state
        .reset_to(repo, "HEAD", git_engine::ResetMode::Hard)
        .unwrap();

    let entry = &state.safety_log()[0];
    assert!(!entry.undoable, "{entry:?}");
}

// The journal kept the commit the tag pointed at, so Undo made a lightweight tag there:
// the message, the tagger and any signature were gone.
#[test]
fn undoing_the_deletion_of_an_annotated_tag_brings_the_annotation_back() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "--annotate", "v1", "--message", "first release"])
        .unwrap();
    let (state, repo) = open(&f);

    state.delete_tag(repo, "v1").unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(
        f.git(&["cat-file", "-t", "refs/tags/v1"]).unwrap().trim(),
        "tag"
    );
    let message = f
        .git(&["tag", "--list", "--format=%(contents:subject)", "v1"])
        .unwrap();
    assert_eq!(message.trim(), "first release");
}

// `rev-parse feature/login` prefers a tag of the same name to the branch, so the journal
// kept the tag's commit and Undo recreated the finished branch in the wrong place.
#[test]
fn undoing_a_finished_feature_restores_the_branch_not_a_tag_of_that_name() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    state
        .flow_init(repo, &git_engine::FlowConfig::default())
        .unwrap();
    f.git(&["tag", "feature/login", "HEAD~1"]).unwrap();
    state
        .flow_start(repo, git_engine::FlowKind::Feature, "login")
        .unwrap();
    std::fs::write(f.path().join("login.rs"), "fn login() {}\n").unwrap();
    f.git(&["add", "--", "login.rs"]).unwrap();
    f.git(&["commit", "-m", "add login"]).unwrap();
    let tip = f.git(&["rev-parse", "refs/heads/feature/login"]).unwrap();

    state
        .flow_finish(repo, git_engine::FlowKind::Feature, "login", None)
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(
        f.git(&["rev-parse", "refs/heads/feature/login"]).unwrap(),
        tip
    );
}

/// `main` and `side` both change `c.txt`, so merging `side` stops on a conflict.
fn about_to_conflict() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-q", "-c", "side"]).unwrap();
    f.commit_file(2, "c.txt", "side\n").unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.commit_file(3, "c.txt", "main\n").unwrap();
    f
}

// The merge was recorded only when it succeeded. One that stopped on a conflict and was
// finished by a commit after the user resolved it left nothing to undo.
#[test]
fn a_merge_finished_after_its_conflict_can_be_undone() {
    let f = about_to_conflict();
    let (state, repo) = open(&f);
    let before = head_oid(&state, repo);
    assert!(
        state
            .merge(
                repo,
                &git_engine::MergeOptions {
                    source: "side".to_owned(),
                    no_fast_forward: false,
                    squash: false,
                    message: None,
                },
            )
            .is_err()
    );
    f.write_file("c.txt", "resolved\n").unwrap();
    f.git(&["add", "c.txt"]).unwrap();
    f.git(&["commit", "-q", "--no-edit"]).unwrap();
    assert_ne!(head_oid(&state, repo), before);

    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), before);
}

// A pull merges or rebases like the toolbar's Merge does, and was not recorded at all.
#[test]
fn a_pull_can_be_undone() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = open(&f);
    let before = head_oid(&state, repo);
    state.pull(repo, "origin", false, |_| {}).unwrap();
    assert_ne!(head_oid(&state, repo), before);

    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), before);
}

fn command_output(err: &git_engine::GitError) -> String {
    match err {
        git_engine::GitError::Command(command) => format!("{}{}", command.stdout, command.stderr),
        other => panic!("expected git's own output, got {other:?}"),
    }
}

// `stash apply --index` stopped on the conflict and wrote its markers; the retry without
// `--index` then failed with "needs merge", and that was all the user saw.
#[test]
fn undoing_a_discard_over_a_conflicting_commit_shows_the_conflict() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    f.write_file("file0.txt", "work in progress\n").unwrap();
    state
        .discard_paths(repo, &["file0.txt".to_owned()])
        .unwrap();
    f.commit_file(5, "file0.txt", "committed meanwhile\n")
        .unwrap();

    let err = state.undo_last(repo).unwrap_err();

    assert!(command_output(&err).contains("CONFLICT"), "{err:?}");
}

fn merge_side(state: &AppState, repo: RepoId) -> Result<(), git_engine::GitError> {
    state.merge(
        repo,
        &git_engine::MergeOptions {
            source: "side".to_owned(),
            no_fast_forward: false,
            squash: false,
            message: None,
        },
    )
}

// `reset --keep` refuses in the middle of a merge and `branch --force` refuses a branch a
// rebase has checked out: Undo failed with git's refusal and no word of what to do.
#[test]
fn undo_waits_for_a_merge_stopped_on_its_conflict() {
    let f = about_to_conflict();
    let (state, repo) = open(&f);
    assert!(merge_side(&state, repo).is_err());

    let err = state.undo_last(repo).unwrap_err();

    assert!(
        matches!(&err, git_engine::GitError::InvalidState(why) if why.contains("abort")),
        "{err:?}"
    );
    assert!(
        f.git_dir().join("MERGE_HEAD").exists(),
        "the merge is left alone"
    );
    assert!(
        state.safety_log()[0].undoable,
        "Undo still works once it is over"
    );
}

#[test]
fn undo_waits_for_a_rebase_stopped_on_its_conflict() {
    let f = about_to_conflict();
    let (state, repo) = open(&f);
    let rebased = state.rebase(
        repo,
        &git_engine::RebaseOptions {
            onto: "side".to_owned(),
            autostash: false,
        },
    );
    assert!(rebased.is_err());

    let err = state.undo_last(repo).unwrap_err();

    assert!(
        matches!(&err, git_engine::GitError::InvalidState(why) if why.contains("abort")),
        "{err:?}"
    );
}

fn text(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

// The rollback leaves the paths changed, so applying the stash of the work it replaced
// was refused with "would be overwritten by merge" every time.
#[test]
fn undoing_a_rollback_brings_back_the_work_it_replaced() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "a.txt", "v1\n").unwrap();
    f.commit_file(3, "a.txt", "v2\n").unwrap();
    let (state, repo) = open(&f);
    f.write_file("a.txt", "work in progress\n").unwrap();

    state
        .rollback_to(repo, "HEAD~1", &["a.txt".to_owned()])
        .unwrap();
    assert_eq!(text(&f, "a.txt"), "v1\n");
    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "a.txt"), "work in progress\n");
}

#[test]
fn a_rollback_of_a_clean_file_can_be_undone_too() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "a.txt", "v1\n").unwrap();
    f.commit_file(3, "a.txt", "v2\n").unwrap();
    let (state, repo) = open(&f);

    state
        .rollback_to(repo, "HEAD~1", &["a.txt".to_owned()])
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "a.txt"), "v2\n");
}

// Undo applied the dropped stash to the working tree instead of listing it again.
#[test]
fn undoing_a_stash_drop_lists_the_stash_again_in_its_place() {
    let f = test_fixtures::with_stashes(3).unwrap();
    let (state, repo) = open(&f);
    let before = state.stashes(repo).unwrap();

    state.stash_drop(repo, 1).unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(state.stashes(repo).unwrap(), before);
    assert!(
        f.git(&["status", "--porcelain"]).unwrap().is_empty(),
        "the working tree is left alone"
    );
}

// Undo applied the stash taken at the old tip on top of the new one: the branch stayed
// where the reset put it and the file got conflict markers.
#[test]
fn undoing_a_hard_reset_puts_the_branch_back_with_the_work_on_it() {
    let f = test_fixtures::linear(1).unwrap();
    let target = f.commit_file(2, "f.txt", "v1\n").unwrap();
    let tip = f.commit_file(3, "f.txt", "v2\n").unwrap();
    let (state, repo) = open(&f);
    f.write_file("f.txt", "v2 and work\n").unwrap();

    state
        .reset_to(repo, &target, git_engine::ResetMode::Hard)
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), tip);
    assert_eq!(text(&f, "f.txt"), "v2 and work\n");
}

#[test]
fn undoing_a_mixed_reset_puts_the_branch_back() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let tip = head_oid(&state, repo);

    state
        .reset_to(repo, "HEAD~2", git_engine::ResetMode::Mixed)
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), tip);
    assert!(f.git(&["status", "--porcelain"]).unwrap().is_empty());
}

#[test]
fn undoing_a_soft_reset_puts_the_branch_back() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let tip = head_oid(&state, repo);

    state
        .reset_to(repo, "HEAD~2", git_engine::ResetMode::Soft)
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), tip);
    assert!(f.git(&["status", "--porcelain"]).unwrap().is_empty());
}

// The rebase returned early on its conflict, before anything was recorded: once the user
// resolved it and continued, there was nothing to undo.
#[test]
fn an_interactive_rebase_finished_after_its_conflict_can_be_undone() {
    let f = test_fixtures::linear(1).unwrap();
    let base = f.commit_file(2, "c.txt", "a\n").unwrap();
    let middle = f.commit_file(3, "c.txt", "b\n").unwrap();
    let tip = f.commit_file(4, "c.txt", "c\n").unwrap();
    let (state, repo) = open(&f);
    let plan = [
        git_engine::TodoEntry {
            oid: middle,
            action: git_engine::TodoAction::Drop,
            message: None,
        },
        git_engine::TodoEntry {
            oid: tip.clone(),
            action: git_engine::TodoAction::Pick,
            message: None,
        },
    ];
    assert!(state.interactive_rebase(repo, &base, &plan, false).is_err());
    f.write_file("c.txt", "c\n").unwrap();
    f.git(&["add", "c.txt"]).unwrap();
    state.continue_operation(repo).unwrap();
    assert_ne!(head_oid(&state, repo), tip);

    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), tip);
}

#[test]
fn an_author_edit_can_be_undone() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let tip = head_oid(&state, repo);

    state
        .edit_author(repo, "HEAD~1", "Someone Else", "else@example.com")
        .unwrap();
    assert_ne!(head_oid(&state, repo), tip);
    state.undo_last(repo).unwrap();

    assert_eq!(head_oid(&state, repo), tip);
}

fn staged(f: &test_fixtures::Fixture, name: &str) -> String {
    f.git(&["show", &format!(":{name}")]).unwrap()
}

// A stash with paths takes their staged side with it: Discard in Unstaged threw away the
// staged edit too, which the confirmation promises to keep.
#[test]
fn discarding_keeps_the_staged_part_of_a_file_and_undo_brings_back_the_rest() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "content 0\nstaged\n").unwrap();
    f.write_file("fresh.txt", "new\n").unwrap();
    f.git(&["add", "--", "file0.txt", "fresh.txt"]).unwrap();
    f.write_file("file0.txt", "content 0\nstaged\nunstaged\n")
        .unwrap();
    f.write_file("fresh.txt", "new\nmore\n").unwrap();
    let (state, repo) = open(&f);
    let paths = ["file0.txt".to_owned(), "fresh.txt".to_owned()];

    state.discard_paths(repo, &paths).unwrap();

    assert_eq!(text(&f, "file0.txt"), "content 0\nstaged\n");
    assert_eq!(staged(&f, "file0.txt"), "content 0\nstaged\n");
    assert_eq!(text(&f, "fresh.txt"), "new\n");
    assert_eq!(staged(&f, "fresh.txt"), "new\n");

    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "file0.txt"), "content 0\nstaged\nunstaged\n");
    assert_eq!(staged(&f, "file0.txt"), "content 0\nstaged\n");
    assert_eq!(text(&f, "fresh.txt"), "new\nmore\n");
    assert_eq!(staged(&f, "fresh.txt"), "new\n");
}

/// Stopped on the conflict in `c.txt`, with `d.txt` merged cleanly and staged beside it.
fn stopped_beside_a_clean_merge() -> (test_fixtures::Fixture, AppState, RepoId) {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-q", "-c", "side"]).unwrap();
    f.commit_file(2, "c.txt", "side\n").unwrap();
    f.commit_file(3, "d.txt", "from side\n").unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.commit_file(4, "c.txt", "main\n").unwrap();
    let (state, repo) = open(&f);
    assert!(merge_side(&state, repo).is_err());
    (f, state, repo)
}

// `git stash push` refuses while any index entry is unmerged ("needs merge"), so the
// backup failed and the main way out of a failed merge did nothing.
#[test]
fn a_hard_reset_goes_ahead_while_a_merge_is_stopped_on_its_conflict() {
    let (f, state, repo) = stopped_beside_a_clean_merge();
    let head = head_oid(&state, repo);

    state
        .reset_to(repo, &head, git_engine::ResetMode::Hard)
        .unwrap();

    assert_eq!(text(&f, "c.txt"), "main\n");
    assert!(!f.path().join("d.txt").exists());
    assert!(state.working_state(repo).unwrap().conflicted.is_empty());
}

#[test]
fn discarding_beside_a_conflict_keeps_the_conflict() {
    let (f, state, repo) = stopped_beside_a_clean_merge();
    f.write_file("file0.txt", "work in progress\n").unwrap();

    state
        .discard_paths(repo, &["file0.txt".to_owned()])
        .unwrap();

    assert_eq!(text(&f, "file0.txt"), "content 0\n");
    assert!(text(&f, "c.txt").contains("<<<<<<<"));
    assert_eq!(state.working_state(repo).unwrap().conflicted, ["c.txt"]);
}

#[test]
fn a_rollback_beside_a_conflict_goes_ahead() {
    let (f, state, repo) = stopped_beside_a_clean_merge();
    f.write_file("file0.txt", "work in progress\n").unwrap();

    state
        .rollback_to(repo, "HEAD", &["file0.txt".to_owned()])
        .unwrap();

    assert_eq!(text(&f, "file0.txt"), "content 0\n");
    assert_eq!(state.working_state(repo).unwrap().conflicted, ["c.txt"]);
}

// Take Ours wrote the stage over the file and staged it with no journal entry: hand edits
// made in an editor during the conflict were gone, with nothing to undo (INV-12).
#[test]
fn taking_one_side_of_a_conflict_can_be_undone_hand_edits_and_all() {
    let f = about_to_conflict();
    let (state, repo) = open(&f);
    assert!(merge_side(&state, repo).is_err());
    f.write_file("c.txt", "half resolved by hand\n").unwrap();

    state
        .resolve_conflict(repo, "c.txt", git_engine::ConflictSide::Ours)
        .unwrap();
    assert_eq!(text(&f, "c.txt"), "main\n");
    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "c.txt"), "half resolved by hand\n");
    assert_eq!(state.working_state(repo).unwrap().conflicted, ["c.txt"]);
}

#[test]
fn saving_a_merge_over_hand_edits_can_be_undone() {
    let f = about_to_conflict();
    let (state, repo) = open(&f);
    assert!(merge_side(&state, repo).is_err());
    f.write_file("c.txt", "half resolved by hand\n").unwrap();

    state
        .resolve_conflict_text(repo, "c.txt", "merged\n")
        .unwrap();
    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "c.txt"), "half resolved by hand\n");
}

/// Stands in for the Recycle Bin: the shell's own move is tested in `src-tauri`.
fn thrown_away(paths: &[std::path::PathBuf]) -> std::io::Result<()> {
    paths.iter().try_for_each(|path| {
        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    })
}

// Delete moved a changed tracked file to the bin with nothing in the journal: its edits
// were only in the bin, and Undo knew nothing of them (F-071).
#[test]
fn deleting_a_changed_file_can_be_undone() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "work in progress\n").unwrap();
    f.write_file("scratch.txt", "not added yet\n").unwrap();
    let (state, repo) = open(&f);
    let paths = ["file0.txt".to_owned(), "scratch.txt".to_owned()];

    state.move_to_trash(repo, &paths, thrown_away).unwrap();
    assert!(!f.path().join("file0.txt").exists());
    state.undo_last(repo).unwrap();

    assert_eq!(text(&f, "file0.txt"), "work in progress\n");
    assert_eq!(text(&f, "scratch.txt"), "not added yet\n");
}

#[test]
fn undoing_a_delete_never_writes_over_a_file_made_since() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("scratch.txt", "first\n").unwrap();
    let (state, repo) = open(&f);
    state
        .move_to_trash(repo, &["scratch.txt".to_owned()], thrown_away)
        .unwrap();
    f.write_file("scratch.txt", "made again\n").unwrap();

    assert!(state.undo_last(repo).is_err());
    assert_eq!(text(&f, "scratch.txt"), "made again\n");
}

#[test]
fn a_deleted_folder_is_left_to_the_bin() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("generated/out.txt", "built\n").unwrap();
    let (state, repo) = open(&f);

    state
        .move_to_trash(repo, &["generated/".to_owned()], thrown_away)
        .unwrap();

    assert!(!f.path().join("generated").exists());
    assert!(state.undo_last(repo).is_err());
    assert!(!state.safety_log().is_empty());
}

// `git branch -d` drops the branch's section from the config, and Undo recreated only the
// name and the commit: ahead/behind and Pull were gone from it.
#[test]
fn undoing_a_branch_delete_brings_its_upstream_back() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["branch", "--track", "topic", "origin/main"])
        .unwrap();
    let (state, repo) = open(&f);

    state.delete_branch(repo, "topic", true).unwrap();
    state.undo_last(repo).unwrap();

    let upstream = f.git(&["config", "--get", "branch.topic.merge"]).ok();
    assert_eq!(upstream.as_deref().map(str::trim), Some("refs/heads/main"));
}
