// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use fs_watcher::{ChangeKind, RepoChanged, RepoWatcher};
use std::sync::mpsc;
use std::time::Duration;

const SETTLE: Duration = Duration::from_millis(600);

struct Harness {
    _dir: tempfile::TempDir,
    watcher: RepoWatcher,
    events: mpsc::Receiver<RepoChanged>,
    root: std::path::PathBuf,
}

fn start() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let git_dir = root.join(".git");
    std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
    std::fs::create_dir_all(git_dir.join("objects")).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(git_dir.join("index"), "").unwrap();

    let (tx, events) = mpsc::channel();
    let watcher = RepoWatcher::start(&root, &git_dir, &git_dir, move |change| {
        let _ = tx.send(change);
    })
    .unwrap();

    // Events from the watcher settling in are not user activity.
    std::thread::sleep(Duration::from_millis(300));
    while events.try_recv().is_ok() {}

    Harness {
        _dir: dir,
        watcher,
        events,
        root,
    }
}

fn collect(harness: &Harness) -> Vec<RepoChanged> {
    let mut seen = Vec::new();
    while let Ok(change) = harness.events.recv_timeout(SETTLE) {
        seen.push(change);
    }
    seen
}

/// The control after a test that expects silence: a watcher that stopped hearing anything,
/// as one does after an overflow, would pass it too.
fn still_hears(harness: &Harness) {
    std::fs::write(
        harness.root.join("control.txt"),
        "edited
",
    )
    .unwrap();
    let seen = collect(harness);
    let heard = seen
        .iter()
        .filter(|c| c.kind == ChangeKind::WorkingTree)
        .count();
    assert_eq!(heard, 1, "the watcher went deaf, got {seen:?}");
}

#[test]
fn a_change_to_head_arrives_as_a_head_event() {
    let harness = start();

    std::fs::write(harness.root.join(".git/HEAD"), "ref: refs/heads/other\n").unwrap();

    let seen = collect(&harness);
    assert!(
        seen.iter().any(|c| c.kind == ChangeKind::Head),
        "got {seen:?}"
    );
}

#[test]
fn a_thousand_files_in_target_produce_no_events() {
    let harness = start();
    let target = harness.root.join("target");
    std::fs::create_dir_all(&target).unwrap();

    for i in 0..1000 {
        std::fs::write(target.join(format!("artifact-{i}.o")), "x").unwrap();
    }

    let seen = collect(&harness);
    assert!(
        seen.is_empty(),
        "INV-06: build output must be filtered out, got {} events",
        seen.len()
    );
    still_hears(&harness);
}

#[test]
fn churn_in_git_objects_is_ignored() {
    let harness = start();
    let objects = harness.root.join(".git/objects/ab");
    std::fs::create_dir_all(&objects).unwrap();

    for i in 0..200 {
        std::fs::write(objects.join(format!("{i:038x}")), "loose object").unwrap();
    }

    assert!(
        collect(&harness).is_empty(),
        "a fetch writes thousands of loose objects and none of them matter"
    );
    still_hears(&harness);
}

#[test]
fn a_new_ref_is_reported_as_a_refs_change() {
    let harness = start();

    std::fs::write(
        harness.root.join(".git/refs/heads/topic"),
        "0000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let seen = collect(&harness);
    assert!(
        seen.iter().any(|c| c.kind == ChangeKind::Refs),
        "got {seen:?}"
    );
}

#[test]
fn a_working_tree_edit_is_reported() {
    let harness = start();

    std::fs::write(harness.root.join("source.txt"), "edited\n").unwrap();

    let seen = collect(&harness);
    assert!(
        seen.iter().any(|c| c.kind == ChangeKind::WorkingTree),
        "got {seen:?}"
    );
}

#[test]
fn an_edited_mailmap_is_reported_as_one() {
    let harness = start();

    std::fs::write(harness.root.join(".mailmap"), "Ann <ann@x> <a@x>\n").unwrap();

    let seen = collect(&harness);
    assert!(
        seen.iter().any(|c| c.kind == ChangeKind::Mailmap),
        "got {seen:?}"
    );
}

#[test]
fn a_paused_watcher_reports_nothing() {
    let harness = start();
    harness.watcher.pause();

    std::fs::write(harness.root.join("source.txt"), "our own mutation\n").unwrap();

    assert!(
        collect(&harness).is_empty(),
        "Cogit must not react to its own writes"
    );
}

#[test]
fn resuming_starts_reporting_again() {
    let harness = start();
    harness.watcher.pause();
    std::fs::write(harness.root.join("a.txt"), "ignored\n").unwrap();
    std::thread::sleep(Duration::from_millis(400));
    while harness.events.try_recv().is_ok() {}
    harness.watcher.resume();

    std::fs::write(harness.root.join("b.txt"), "seen\n").unwrap();

    assert!(!collect(&harness).is_empty());
}

#[test]
fn a_missing_repository_fails_instead_of_panicking() {
    let missing = std::path::Path::new("C:/no/such/repository/anywhere");

    let result = RepoWatcher::start(
        missing,
        &missing.join(".git"),
        &missing.join(".git"),
        |_| {},
    );

    assert!(result.is_err());
}

#[test]
fn events_inside_a_quiet_window_are_dropped() {
    let harness = start();
    harness.watcher.quiet_for(Duration::from_millis(700));

    std::fs::write(harness.root.join("ours.txt"), "written by Cogit\n").unwrap();

    assert!(
        collect(&harness).is_empty(),
        "our own writes arrive debounced, after the command has already finished"
    );
}

#[test]
fn events_after_the_quiet_window_still_arrive() {
    let harness = start();
    harness.watcher.quiet_for(Duration::from_millis(150));
    std::thread::sleep(Duration::from_millis(400));
    while harness.events.try_recv().is_ok() {}

    std::fs::write(harness.root.join("theirs.txt"), "written in a terminal\n").unwrap();

    assert!(!collect(&harness).is_empty());
}

#[test]
fn a_later_quiet_window_extends_an_earlier_one() {
    let harness = start();
    harness.watcher.quiet_for(Duration::from_millis(200));
    harness.watcher.quiet_for(Duration::from_millis(900));

    std::fs::write(harness.root.join("ours.txt"), "second mutation\n").unwrap();

    assert!(collect(&harness).is_empty());
}

#[test]
fn a_shorter_window_does_not_cut_a_longer_one_short() {
    let harness = start();
    harness.watcher.quiet_for(Duration::from_millis(900));
    harness.watcher.quiet_for(Duration::from_millis(50));

    std::fs::write(harness.root.join("ours.txt"), "still ours\n").unwrap();

    assert!(collect(&harness).is_empty());
}

#[test]
fn a_hundred_files_at_once_do_not_become_a_hundred_events() {
    let harness = start();

    for i in 0..100 {
        std::fs::write(harness.root.join(format!("file-{i}.txt")), "x").unwrap();
    }

    let seen = collect(&harness);
    assert!(
        !seen.is_empty(),
        "the burst still has to be announced, just not once per file"
    );
    assert!(
        seen.len() <= 4,
        "a checkout must not redraw the panel once per file, got {} events",
        seen.len()
    );
    assert!(seen.iter().all(|c| c.kind == ChangeKind::WorkingTree));
}

// In a linked worktree the refs live in the common git directory, and only the private
// one was watched: a commit or a fetch from the terminal never refreshed Branches.
#[test]
fn a_linked_worktree_hears_about_refs_in_the_common_directory() {
    let dir = tempfile::tempdir().unwrap();
    let common = dir.path().join("main/.git");
    let private = common.join("worktrees/linked");
    let root = dir.path().join("linked");
    std::fs::create_dir_all(common.join("refs/heads")).unwrap();
    std::fs::create_dir_all(&private).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(private.join("HEAD"), "ref: refs/heads/wt\n").unwrap();

    let (tx, events) = mpsc::channel();
    let _watcher = RepoWatcher::start(&root, &private, &common, move |change| {
        let _ = tx.send(change);
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    while events.try_recv().is_ok() {}

    std::fs::write(
        common.join("refs/heads/main"),
        "0000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let mut seen = Vec::new();
    while let Ok(change) = events.recv_timeout(SETTLE) {
        seen.push(change);
    }
    assert!(seen.iter().any(|c| c.kind == ChangeKind::Refs), "{seen:?}");
}

// A submodule keeps its git directory under the parent's `.git/modules`, outside its own
// root; hooks were only ever heard through the root's recursive watch, so an edit to a
// submodule's hook never reached the Hooks panel.
#[test]
fn a_hook_edited_in_a_git_directory_outside_the_root_is_heard() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join("parent/.git/modules/sub");
    let root = dir.path().join("parent/sub");
    std::fs::create_dir_all(git_dir.join("hooks")).unwrap();
    std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let (tx, events) = mpsc::channel();
    let _watcher = RepoWatcher::start(&root, &git_dir, &git_dir, move |change| {
        let _ = tx.send(change);
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    while events.try_recv().is_ok() {}

    std::fs::write(git_dir.join("hooks/pre-commit"), "#!/bin/sh\n").unwrap();

    let mut seen = Vec::new();
    while let Ok(change) = events.recv_timeout(SETTLE) {
        seen.push(change);
    }
    assert!(seen.iter().any(|c| c.kind == ChangeKind::Hooks), "{seen:?}");
}

// Windows refuses to rename a folder while a handle is open on anything inside it: the
// watches of their own on .git and .git/refs kept the repository's folder from being
// renamed, moved or put in the Recycle Bin, with the repository not even on screen.
#[test]
fn a_watched_repository_folder_can_still_be_renamed() {
    let harness = start();
    let moved = harness.root.with_extension("moved");

    let renamed = std::fs::rename(&harness.root, &moved);
    if renamed.is_ok() {
        std::fs::rename(&moved, &harness.root).unwrap();
    }

    renamed.unwrap();
}

// The window was a timer re-armed per git process: one process running longer than it
// (a commit whose pre-commit hook sleeps) wrote the index into a closed window (R-445).
#[test]
fn a_held_watcher_stays_quiet_however_long_the_mutation_takes() {
    let harness = start();
    let hold = harness.watcher.hold();
    std::thread::sleep(fs_watcher::DEFAULT_QUIET + Duration::from_millis(300));

    std::fs::write(
        harness.root.join("ours.txt"),
        "written late in the mutation\n",
    )
    .unwrap();

    assert!(collect(&harness).is_empty());
    drop(hold);
}

#[test]
fn letting_go_keeps_the_window_open_for_the_debounced_tail() {
    let harness = start();
    let hold = harness.watcher.hold();
    std::fs::write(harness.root.join("ours.txt"), "the last write\n").unwrap();
    drop(hold);

    assert!(collect(&harness).is_empty());
    std::fs::write(harness.root.join("theirs.txt"), "written in a terminal\n").unwrap();
    assert!(!collect(&harness).is_empty());
}
