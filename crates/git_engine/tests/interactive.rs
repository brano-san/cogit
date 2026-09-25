// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, TodoAction, TodoEntry, render_todo};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn entry(oid: &str, action: TodoAction) -> TodoEntry {
    TodoEntry {
        oid: oid.to_owned(),
        action,
        message: None,
    }
}

fn subjects(f: &test_fixtures::Fixture) -> Vec<String> {
    f.git(&["log", "--format=%s"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_plan_of_picks_renders_one_line_each_in_order() {
    let plan = [
        entry("aaaa", TodoAction::Pick),
        entry("bbbb", TodoAction::Pick),
    ];

    assert_eq!(render_todo(&plan), "pick aaaa\npick bbbb\n");
}

#[test]
fn every_action_has_the_verb_git_expects() {
    let plan = [
        entry("a", TodoAction::Pick),
        entry("b", TodoAction::Reword),
        entry("c", TodoAction::Edit),
        entry("d", TodoAction::Squash),
        entry("e", TodoAction::Fixup),
        entry("f", TodoAction::Drop),
    ];

    let todo = render_todo(&plan);

    assert!(todo.contains("pick a\n"), "{todo}");
    assert!(
        todo.contains("reword b\n") || todo.contains("pick b\n"),
        "{todo}"
    );
    assert!(todo.contains("edit c\n"), "{todo}");
    assert!(todo.contains("squash d\n"), "{todo}");
    assert!(todo.contains("fixup e\n"), "{todo}");
    assert!(todo.contains("drop f\n"), "{todo}");
}

#[test]
fn a_new_message_becomes_an_amend_step_rather_than_an_editor_prompt() {
    let plan = [TodoEntry {
        oid: "aaaa".to_owned(),
        action: TodoAction::Reword,
        message: Some("a better subject".to_owned()),
    }];

    let todo = render_todo(&plan);

    assert!(todo.contains("exec git commit --amend"), "{todo}");
    assert!(todo.contains("a better subject"), "{todo}");
}

#[test]
fn a_quote_in_a_message_cannot_break_out_of_the_exec_line() {
    let plan = [TodoEntry {
        oid: "aaaa".to_owned(),
        action: TodoAction::Reword,
        message: Some("it's here; rm -rf /".to_owned()),
    }];

    let todo = render_todo(&plan);
    let line = todo.lines().find(|l| l.starts_with("exec")).unwrap();

    assert!(line.ends_with('\''), "{line}");
    assert!(
        !line.contains("' "),
        "an unescaped quote ends the argument: {line}"
    );
}

#[test]
fn reordering_two_commits_changes_their_order() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let last = f.oid("HEAD").unwrap();
    let first = f.oid("HEAD~1").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&last, TodoAction::Pick),
                entry(&first, TodoAction::Pick),
            ],
        )
        .unwrap();

    assert_eq!(subjects(&f)[0], "commit 2", "{:?}", subjects(&f));
}

#[test]
fn squashing_leaves_one_commit_where_there_were_two() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let before = f.git(&["rev-list", "--count", "HEAD"]).unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
                entry(&f.oid("HEAD").unwrap(), TodoAction::Squash),
            ],
        )
        .unwrap();

    let after = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() - 1
    );
}

#[test]
fn dropping_a_commit_removes_it_from_the_history() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&f.oid("HEAD~1").unwrap(), TodoAction::Drop),
                entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
            ],
        )
        .unwrap();

    assert!(
        !subjects(&f).contains(&"commit 2".to_owned()),
        "{:?}",
        subjects(&f)
    );
}

#[test]
fn rewording_replaces_the_subject() {
    let f = test_fixtures::linear(3).unwrap();
    let base = f.oid("HEAD~1").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[TodoEntry {
                oid: f.oid("HEAD").unwrap(),
                action: TodoAction::Reword,
                message: Some("a much better subject".to_owned()),
            }],
        )
        .unwrap();

    assert_eq!(subjects(&f)[0], "a much better subject");
}

// Native `git rebase -i` runs commit-msg on a reword; Cogit's exec line had --no-verify,
// so a Conventional Commits hook never saw the new message and nothing recorded the bypass.
#[test]
fn a_commit_msg_hook_checks_a_reworded_message() {
    let f = test_fixtures::linear(3).unwrap();
    let hooks = f.path().join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("commit-msg");
    std::fs::write(
        &hook,
        "#!/bin/sh\necho 'not a conventional subject' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut mode = std::fs::metadata(&hook).unwrap().permissions();
        mode.set_mode(0o755);
        std::fs::set_permissions(&hook, mode).unwrap();
    }
    let base = f.oid("HEAD~1").unwrap();

    let result = open(&f).interactive_rebase(
        &base,
        &[TodoEntry {
            oid: f.oid("HEAD").unwrap(),
            action: TodoAction::Reword,
            message: Some("stuff".to_owned()),
        }],
    );

    let Err(git_engine::GitError::Command(failure)) = result else {
        panic!("the hook refused the message, so the rebase must stop: {result:?}");
    };
    assert!(
        failure.stderr.contains("not a conventional subject"),
        "{failure:?}"
    );
}

#[test]
fn an_empty_plan_is_refused() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(
        open(&f)
            .interactive_rebase(&f.oid("HEAD~1").unwrap(), &[])
            .is_err()
    );
}

#[test]
fn a_dirty_working_tree_stops_the_rebase_before_it_starts() {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("file0.txt", "uncommitted\n").unwrap();

    assert!(
        open(&f)
            .interactive_rebase(
                &f.oid("HEAD~1").unwrap(),
                &[entry(&f.oid("HEAD").unwrap(), TodoAction::Pick)],
            )
            .is_err()
    );
}

#[test]
fn the_default_plan_lists_the_range_oldest_first_as_picks() {
    let f = test_fixtures::linear(4).unwrap();

    let plan = open(&f).rebase_todo(&f.oid("HEAD~2").unwrap()).unwrap();

    assert_eq!(plan.len(), 2);
    assert!(plan.iter().all(|e| matches!(e.action, TodoAction::Pick)));
    assert_eq!(plan[0].oid, f.oid("HEAD~1").unwrap());
}

#[test]
fn the_default_plan_carries_each_commit_subject_for_the_editor() {
    let f = test_fixtures::linear(3).unwrap();

    let plan = open(&f).rebase_todo(&f.oid("HEAD~1").unwrap()).unwrap();

    assert_eq!(plan[0].message.as_deref(), Some("commit 2"));
}

#[test]
fn pausing_after_each_commit_inserts_a_break_between_them() {
    let plan = [
        entry("aaaa", TodoAction::Pick),
        entry("bbbb", TodoAction::Pick),
    ];

    let todo = git_engine::render_todo_paused(&plan);

    assert_eq!(todo, "pick aaaa\nbreak\npick bbbb\nbreak\n");
}

#[test]
fn a_dropped_commit_gets_no_break_because_nothing_was_applied() {
    let plan = [
        entry("aaaa", TodoAction::Drop),
        entry("bbbb", TodoAction::Pick),
    ];

    assert_eq!(
        git_engine::render_todo_paused(&plan),
        "drop aaaa\npick bbbb\nbreak\n"
    );
}

#[test]
fn an_edit_step_gets_no_extra_break_because_it_already_stops() {
    let plan = [entry("aaaa", TodoAction::Edit)];
    assert_eq!(git_engine::render_todo_paused(&plan), "edit aaaa\n");
}

#[test]
fn a_paused_rebase_stops_before_it_has_finished() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let plan = [
        entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
        entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
    ];

    open(&f).interactive_rebase_paused(&base, &plan).unwrap();

    assert!(f.path().join(".git/rebase-merge").is_dir());
}

#[test]
fn continuing_a_paused_rebase_reaches_the_end() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let plan = [
        entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
        entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
    ];
    let repo = open(&f);
    repo.interactive_rebase_paused(&base, &plan).unwrap();

    while f.path().join(".git/rebase-merge").is_dir() {
        repo.continue_operation().unwrap();
    }

    assert_eq!(subjects(&f)[0], "commit 3", "{:?}", subjects(&f));
}

fn message(f: &test_fixtures::Fixture, rev: &str) -> String {
    f.git(&["log", "-1", "--format=%B", rev])
        .unwrap()
        .trim_end()
        .to_owned()
}

// Edit Message sends the whole message: its line breaks went into the todo as they were,
// git refused the todo, and the repository was left in the middle of a rebase.
#[test]
fn rewording_with_a_message_of_several_lines_keeps_every_line() {
    let f = test_fixtures::linear(2).unwrap();
    let base = f.oid("HEAD~1").unwrap();
    let repo = open(&f);

    repo.interactive_rebase(
        &base,
        &[TodoEntry {
            oid: f.oid("HEAD").unwrap(),
            action: TodoAction::Reword,
            message: Some("Subject\n\nFirst paragraph.\nIt's two lines.".to_owned()),
        }],
    )
    .unwrap();

    assert_eq!(
        message(&f, "HEAD"),
        "Subject\n\nFirst paragraph.\nIt's two lines."
    );
    assert_eq!(repo.state().unwrap(), git_engine::RepoState::Clean);
}

// The editor offers the subject alone to change; the body went with the old subject.
#[test]
fn rewording_the_subject_keeps_the_body() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(
        "worded.txt",
        "w
",
    )
    .unwrap();
    f.git(&["add", "worded.txt"]).unwrap();
    f.git(&["commit", "-q", "-m", "Old subject", "-m", "The body."])
        .unwrap();
    let base = f.oid("HEAD~1").unwrap();
    let repo = open(&f);
    let mut plan = repo.rebase_todo(&base).unwrap();
    plan[0].action = TodoAction::Reword;
    plan[0].message = Some("New subject".to_owned());

    repo.interactive_rebase(&base, &plan).unwrap();

    assert_eq!(message(&f, "HEAD"), "New subject\n\nThe body.");
}

// Marking a row Squash in the editor left its subject in the entry, and that one line
// replaced the message git had combined from both commits.
#[test]
fn squashing_from_the_default_plan_keeps_both_messages() {
    let f = test_fixtures::linear(3).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let repo = open(&f);
    let mut plan = repo.rebase_todo(&base).unwrap();
    plan[1].action = TodoAction::Squash;

    repo.interactive_rebase(&base, &plan).unwrap();

    let combined = message(&f, "HEAD");
    assert!(
        combined.contains("commit 1") && combined.contains("commit 2"),
        "{combined}"
    );
}

/// base — before — merge of `side` — after, on the checked-out branch.
fn with_a_merge() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "before.txt", "b\n").unwrap();
    f.git(&["checkout", "-q", "-b", "side"]).unwrap();
    f.commit_file(3, "side.txt", "s\n").unwrap();
    f.git(&["checkout", "-q", "-"]).unwrap();
    f.merge(4, &["side"], "merge side").unwrap();
    f.commit_file(5, "after.txt", "a\n").unwrap();
    f
}

// `pick <merge>` stops with "is a merge but no -m option was given".
#[test]
fn the_default_plan_leaves_merge_commits_out_as_git_does() {
    let f = with_a_merge();
    let merge = f.oid("HEAD~1").unwrap();

    let plan = open(&f).rebase_todo(&f.oid("HEAD~3").unwrap()).unwrap();

    assert!(plan.iter().all(|entry| entry.oid != merge), "{plan:?}");
}

#[test]
fn a_new_author_under_a_merge_is_refused_before_anything_moves() {
    let f = with_a_merge();
    let repo = open(&f);
    let head = f.oid("HEAD").unwrap();

    let result = repo.edit_author(&f.oid("HEAD~2").unwrap(), "New", "new@example.com");

    assert!(result.is_err());
    assert_eq!(repo.state().unwrap(), git_engine::RepoState::Clean);
    assert_eq!(f.oid("HEAD").unwrap(), head);
}
