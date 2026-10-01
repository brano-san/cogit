// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Pull and Push dialogs: tags, notes that travel beside branches, leases, and what a
//! repository remembers. Remotes are local bare repositories.

use git_engine::{
    FetchOptions, NetworkDefaults, PullMethod, PullOptions, PushOptions, RepoHandle, TagsMode,
};
use std::path::PathBuf;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn no_token(_url: &str) -> Option<String> {
    None
}

fn bare(f: &test_fixtures::Fixture) -> PathBuf {
    PathBuf::from(
        f.git(&["config", "--get", "remote.origin.url"])
            .unwrap()
            .trim(),
    )
}

fn sync_with_origin(f: &test_fixtures::Fixture) {
    f.git(&["reset", "--hard", "origin/main"]).unwrap();
}

fn push_options(f: impl FnOnce(&mut PushOptions)) -> PushOptions {
    let mut options = PushOptions {
        remote: "origin".into(),
        local: "main".into(),
        branch: "main".into(),
        set_upstream: false,
        tags: TagsMode::None,
        notes: false,
        force_with_lease: false,
    };
    f(&mut options);
    options
}

fn fetch_notes(f: &test_fixtures::Fixture) -> git_engine::NotesFetch {
    open(f)
        .fetch_options(
            "origin",
            FetchOptions {
                tags: false,
                notes: true,
            },
            no_token,
            |_| {},
        )
        .unwrap()
}

#[test]
fn pull_with_tags_fetches_new_tags_and_moves_existing_ones() {
    let f = test_fixtures::with_remote().unwrap();
    let remote = bare(&f);
    let first = f.oid("refs/remotes/origin/main~1").unwrap();
    let tip = f.oid("refs/remotes/origin/main").unwrap();
    f.git_in(&remote, &["tag", "v1", &first]).unwrap();
    let options = PullOptions {
        fetch: FetchOptions {
            tags: true,
            notes: false,
        },
        ..PullOptions::default()
    };

    open(&f)
        .pull_options("origin", options, no_token, |_| {})
        .unwrap();
    assert_eq!(f.oid("refs/tags/v1").unwrap(), first);

    f.git_in(&remote, &["tag", "-f", "v1", &tip]).unwrap();
    open(&f)
        .pull_options("origin", options, no_token, |_| {})
        .unwrap();
    assert_eq!(f.oid("refs/tags/v1").unwrap(), tip);
}

#[test]
fn pull_without_the_tag_option_leaves_a_moved_tag_alone() {
    let f = test_fixtures::with_remote().unwrap();
    let remote = bare(&f);
    let first = f.oid("refs/remotes/origin/main~1").unwrap();
    f.git(&["tag", "v1", &first]).unwrap();
    f.git_in(&remote, &["tag", "v1", "main"]).unwrap();

    open(&f)
        .pull_options("origin", PullOptions::default(), no_token, |_| {})
        .unwrap();

    assert_eq!(f.oid("refs/tags/v1").unwrap(), first);
}

#[test]
fn pull_can_rebase_the_local_branch() {
    let f = test_fixtures::with_remote().unwrap();
    let options = PullOptions {
        method: PullMethod::Rebase,
        ..PullOptions::default()
    };

    open(&f)
        .pull_options("origin", options, no_token, |_| {})
        .unwrap();

    let merges = f.git(&["rev-list", "--merges", "HEAD"]).unwrap();
    assert!(merges.trim().is_empty(), "a rebase leaves no merge commit");
    assert_eq!(
        f.git(&["rev-list", "--count", "origin/main..HEAD"])
            .unwrap()
            .trim(),
        "2"
    );
}

#[test]
fn fetch_only_does_not_touch_the_branch() {
    let f = test_fixtures::with_remote().unwrap();
    let before = f.oid("HEAD").unwrap();

    open(&f)
        .fetch_options("origin", FetchOptions::default(), no_token, |_| {})
        .unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn notes_that_only_the_remote_has_arrive_under_the_same_name() {
    let f = test_fixtures::with_remote().unwrap();
    f.git_in(&bare(&f), &["notes", "add", "-m", "remote note", "main"])
        .unwrap();

    let got = fetch_notes(&f);

    assert!(got.diverged.is_empty());
    assert_eq!(
        f.git(&["notes", "show", "origin/main"]).unwrap().trim(),
        "remote note"
    );
}

#[test]
fn notes_that_fell_behind_fast_forward() {
    let f = test_fixtures::with_remote().unwrap();
    let remote = bare(&f);
    f.git_in(&remote, &["notes", "add", "-m", "one", "main"])
        .unwrap();
    fetch_notes(&f);
    f.git_in(&remote, &["notes", "add", "-f", "-m", "two", "main"])
        .unwrap();

    let got = fetch_notes(&f);

    assert!(got.diverged.is_empty());
    assert_eq!(
        f.git(&["notes", "show", "origin/main"]).unwrap().trim(),
        "two"
    );
}

fn diverged_notes() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::with_remote().unwrap();
    let remote = bare(&f);
    f.git_in(&remote, &["notes", "add", "-m", "remote", "main"])
        .unwrap();
    f.git(&["notes", "add", "-m", "local", "HEAD"]).unwrap();
    let ours = f.oid("refs/notes/commits").unwrap();
    (f, ours)
}

#[test]
fn diverged_notes_are_reported_and_never_overwritten() {
    let (f, ours) = diverged_notes();

    let got = fetch_notes(&f);

    assert_eq!(got.diverged, ["commits"]);
    assert_eq!(got.remote, "origin");
    assert_eq!(f.oid("refs/notes/commits").unwrap(), ours);
    assert!(f.oid("refs/notes-remote/origin/commits").is_ok());
}

#[test]
fn diverged_notes_merge_when_asked() {
    let (f, _) = diverged_notes();
    fetch_notes(&f);

    open(&f).merge_notes("origin", "commits").unwrap();

    assert_eq!(f.git(&["notes", "show", "HEAD"]).unwrap().trim(), "local");
    assert_eq!(
        f.git(&["notes", "show", "origin/main"]).unwrap().trim(),
        "remote"
    );
}

#[test]
fn a_notes_merge_that_conflicts_is_aborted_with_gits_words() {
    let f = test_fixtures::with_remote().unwrap();
    f.git_in(&bare(&f), &["notes", "add", "-m", "theirs", "main~1"])
        .unwrap();
    f.git(&["notes", "add", "-m", "ours", "origin/main~1"])
        .unwrap();
    fetch_notes(&f);

    let err = open(&f).merge_notes("origin", "commits").unwrap_err();

    assert!(matches!(err, git_engine::GitError::Command(_)), "{err:?}");
    assert!(!f.git_dir().join("NOTES_MERGE_REF").exists());
}

#[test]
fn pushing_notes_sends_them_and_remembers_what_the_remote_has() {
    let f = test_fixtures::with_remote().unwrap();
    sync_with_origin(&f);
    f.git(&["notes", "add", "-m", "mine", "HEAD"]).unwrap();

    let sent = open(&f)
        .push_options(&push_options(|o| o.notes = true), no_token, |_| {})
        .unwrap();

    assert_eq!(sent.notes_rejected, None);
    assert_eq!(
        f.git_in(&bare(&f), &["notes", "show", "main"])
            .unwrap()
            .trim(),
        "mine"
    );
    let preview = open(&f).push_preview("main", "origin", "main", 10);
    assert_eq!(preview.notes_unpushed, Some(0));
}

#[test]
fn notes_refused_as_non_fast_forward_are_an_outcome_and_the_branch_still_goes() {
    let f = test_fixtures::with_remote().unwrap();
    sync_with_origin(&f);
    f.git_in(&bare(&f), &["notes", "add", "-m", "remote", "main"])
        .unwrap();
    f.git(&["notes", "add", "-m", "local", "HEAD"]).unwrap();
    f.commit_file(50, "more.txt", "more\n").unwrap();
    let local = f.oid("HEAD").unwrap();

    let sent = open(&f)
        .push_options(&push_options(|o| o.notes = true), no_token, |_| {})
        .unwrap();

    let words = sent.notes_rejected.expect("the notes were refused");
    assert!(words.contains("rejected"), "{words}");
    assert_eq!(
        f.oid("refs/remotes/origin/main").unwrap_or_default(),
        f.oid("origin/main").unwrap()
    );
    assert_eq!(
        f.git_in(&bare(&f), &["rev-parse", "main"]).unwrap().trim(),
        local
    );
}

#[test]
fn a_lease_refuses_to_overwrite_what_arrived_after_the_last_fetch() {
    let f = test_fixtures::with_remote().unwrap();
    let remote = bare(&f);
    let other = remote.parent().unwrap().join("late");
    f.git_in(
        remote.parent().unwrap(),
        &["clone", &remote.to_string_lossy(), &other.to_string_lossy()],
    )
    .unwrap();
    f.git_in(&other, &["commit", "--allow-empty", "-m", "late work"])
        .unwrap();
    f.git_in(&other, &["push"]).unwrap();

    let err = open(&f)
        .push_options(
            &push_options(|o| o.force_with_lease = true),
            no_token,
            |_| {},
        )
        .unwrap_err();

    let git_engine::GitError::Command(failure) = err else {
        panic!("a refused lease is git's failure: {err:?}");
    };
    assert!(
        failure.stderr.contains("stale info") || failure.stderr.contains("rejected"),
        "{failure:?}"
    );
}

#[test]
fn without_a_lease_a_diverged_push_is_refused() {
    let f = test_fixtures::with_remote().unwrap();

    let err = open(&f)
        .push_options(&push_options(|_| {}), no_token, |_| {})
        .unwrap_err();

    assert!(matches!(err, git_engine::GitError::Command(_)));
}

#[test]
fn a_lease_overwrites_a_remote_it_has_seen() {
    let f = test_fixtures::with_remote().unwrap();
    sync_with_origin(&f);
    f.commit_file(80, "x.txt", "x\n").unwrap();
    open(&f)
        .push_options(&push_options(|_| {}), no_token, |_| {})
        .unwrap();
    f.git(&["commit", "--amend", "-m", "rewritten"]).unwrap();
    let local = f.oid("HEAD").unwrap();

    open(&f)
        .push_options(
            &push_options(|o| o.force_with_lease = true),
            no_token,
            |_| {},
        )
        .unwrap();

    assert_eq!(
        f.git_in(&bare(&f), &["rev-parse", "main"]).unwrap().trim(),
        local
    );
}

#[test]
fn a_lease_refuses_a_remote_commit_never_integrated_even_when_fetched() {
    let f = test_fixtures::with_remote().unwrap();

    let err = open(&f)
        .push_options(
            &push_options(|o| o.force_with_lease = true),
            no_token,
            |_| {},
        )
        .unwrap_err();

    assert!(matches!(err, git_engine::GitError::Command(_)));
}

#[test]
fn a_new_branch_is_pushed_and_tracked_with_set_upstream() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["checkout", "-b", "topic"]).unwrap();

    open(&f)
        .push_options(
            &push_options(|o| {
                o.local = "topic".into();
                o.branch = "topic-remote".into();
                o.set_upstream = true;
            }),
            no_token,
            |_| {},
        )
        .unwrap();

    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "topic@{upstream}"])
            .unwrap()
            .trim(),
        "origin/topic-remote"
    );
}

#[test]
fn tag_modes_send_none_the_followed_or_all() {
    let f = test_fixtures::with_remote().unwrap();
    sync_with_origin(&f);
    f.commit_file(60, "t.txt", "t\n").unwrap();
    f.git(&["tag", "-a", "followed", "-m", "x"]).unwrap();
    f.git(&["tag", "light", "HEAD~1"]).unwrap();
    let remote = bare(&f);
    let tags = || f.git_in(&remote, &["tag", "--list"]).unwrap();

    open(&f)
        .push_options(&push_options(|_| {}), no_token, |_| {})
        .unwrap();
    assert_eq!(tags().trim(), "");

    f.commit_file(61, "u.txt", "u\n").unwrap();
    f.git(&["tag", "-a", "second", "-m", "x"]).unwrap();
    open(&f)
        .push_options(
            &push_options(|o| o.tags = TagsMode::Follow),
            no_token,
            |_| {},
        )
        .unwrap();
    assert!(tags().contains("second") && !tags().contains("light"));

    open(&f)
        .push_options(&push_options(|o| o.tags = TagsMode::All), no_token, |_| {})
        .unwrap();
    assert!(tags().contains("light") && tags().contains("followed"));
}

#[test]
fn the_preview_counts_every_commit_but_lists_a_few() {
    let f = test_fixtures::with_remote().unwrap();

    let preview = open(&f).push_preview("main", "origin", "main", 1);

    assert_eq!(preview.total, 2);
    assert_eq!(preview.commits.len(), 1);
    assert_eq!(preview.commits[0].summary, "commit 21");
    assert_eq!(preview.notes_unpushed, None);
    assert!(!preview.has_local_notes);
}

#[test]
fn a_branch_the_remote_lacks_is_measured_against_what_the_remote_has() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["checkout", "-b", "topic", "origin/main"]).unwrap();
    f.commit_file(70, "topic.txt", "x\n").unwrap();

    let preview = open(&f).push_preview("topic", "origin", "topic", 10);

    assert_eq!(preview.total, 1);
}

#[test]
fn unpushed_notes_are_counted_against_the_notes_last_fetched() {
    let f = test_fixtures::with_remote().unwrap();
    f.git_in(&bare(&f), &["notes", "add", "-m", "r", "main~1"])
        .unwrap();
    fetch_notes(&f);
    f.git(&["notes", "add", "-m", "a", "HEAD"]).unwrap();
    f.git(&["notes", "add", "-m", "b", "HEAD~1"]).unwrap();

    let preview = open(&f).push_preview("main", "origin", "main", 10);

    assert!(preview.has_local_notes);
    assert_eq!(preview.notes_unpushed, Some(2));
}

#[test]
fn a_repository_remembers_its_choices_in_its_own_config() {
    let f = test_fixtures::linear(1).unwrap();
    let wanted = NetworkDefaults {
        pull_method: PullMethod::Rebase,
        pull_tags: true,
        pull_notes: true,
        push_tags: TagsMode::Follow,
        push_notes: true,
        push_set_upstream: Some(false),
    };
    assert_eq!(
        open(&f).network_defaults().unwrap(),
        NetworkDefaults::default()
    );

    open(&f).save_network_defaults(&wanted).unwrap();
    assert_eq!(open(&f).network_defaults().unwrap(), wanted);

    let automatic = NetworkDefaults {
        push_set_upstream: None,
        ..wanted
    };
    open(&f).save_network_defaults(&automatic).unwrap();
    assert_eq!(open(&f).network_defaults().unwrap().push_set_upstream, None);
}
