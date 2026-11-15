// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Open in External Diff Tool (F-944): `git difftool` gets the two sides the Files pane shows.

use git_engine::{DiffSpec, GitError, RepoHandle, diff_tool_from_listing, difftool_args};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A recording tool: copies `$LOCAL` and `$REMOTE` beside the git dir, then writes `done`.
fn recorder(f: &test_fixtures::Fixture) -> (String, PathBuf) {
    let dir = f.git_dir().join("recorded");
    std::fs::create_dir_all(&dir).unwrap();
    let at = slashed(&dir);
    let cmd = format!(
        "cat \"$LOCAL\" > '{at}/local' && cat \"$REMOTE\" > '{at}/remote' && echo ok > '{at}/done'"
    );
    (cmd, dir)
}

fn no_tool_configured(f: &test_fixtures::Fixture) {
    f.git(&["config", "diff.tool", ""]).unwrap();
    f.git(&["config", "merge.tool", ""]).unwrap();
}

fn recorded(dir: &Path) -> (String, String) {
    let started = Instant::now();
    while !dir.join("done").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the tool never ran"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    (
        std::fs::read_to_string(dir.join("local")).unwrap(),
        std::fs::read_to_string(dir.join("remote")).unwrap(),
    )
}

#[test]
fn the_working_tree_is_compared_with_the_index() {
    let f = test_fixtures::linear(1).unwrap();
    let (cmd, dir) = recorder(&f);
    f.git(&["config", "diff.tool", "rec"]).unwrap();
    f.git(&["config", "difftool.rec.cmd", &cmd]).unwrap();
    std::fs::write(f.path().join("file0.txt"), "on disk\n").unwrap();

    open(&f)
        .launch_difftool(&DiffSpec::WorkTreeVsIndex, "file0.txt", None, None)
        .unwrap();

    assert_eq!(recorded(&dir), ("content 0\n".into(), "on disk\n".into()));
}

#[test]
fn the_index_is_compared_with_head_through_a_program_of_cogit() {
    let f = test_fixtures::linear(1).unwrap();
    no_tool_configured(&f);
    let (cmd, dir) = recorder(&f);
    std::fs::write(f.path().join("file0.txt"), "staged\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    std::fs::write(f.path().join("file0.txt"), "later\n").unwrap();

    open(&f)
        .launch_difftool(&DiffSpec::IndexVsHead, "file0.txt", None, Some(&cmd))
        .unwrap();

    assert_eq!(recorded(&dir), ("content 0\n".into(), "staged\n".into()));
}

#[test]
fn a_commit_is_compared_with_its_parent() {
    let f = test_fixtures::linear(1).unwrap();
    let oid = f.commit_file(5, "file0.txt", "changed\n").unwrap();
    f.commit_file(6, "file0.txt", "newer\n").unwrap();
    let (cmd, dir) = recorder(&f);

    open(&f)
        .launch_difftool(
            &DiffSpec::CommitVsParent { oid },
            "file0.txt",
            None,
            Some(&cmd),
        )
        .unwrap();

    assert_eq!(recorded(&dir), ("content 0\n".into(), "changed\n".into()));
}

#[test]
fn the_root_commit_is_compared_with_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    let oid = f.oid("HEAD").unwrap();
    let (cmd, dir) = recorder(&f);

    open(&f)
        .launch_difftool(
            &DiffSpec::CommitVsParent { oid },
            "file0.txt",
            None,
            Some(&cmd),
        )
        .unwrap();

    assert_eq!(recorded(&dir), (String::new(), "content 0\n".into()));
}

#[test]
fn without_a_tool_the_error_says_how_to_configure_one() {
    let f = test_fixtures::linear(1).unwrap();
    no_tool_configured(&f);
    std::fs::write(f.path().join("file0.txt"), "on disk\n").unwrap();

    let err = open(&f)
        .launch_difftool(&DiffSpec::WorkTreeVsIndex, "file0.txt", None, None)
        .unwrap_err();

    let GitError::InvalidState(text) = err else {
        panic!("expected a refusal, got {err:?}");
    };
    assert!(
        text.contains("Preferences") && text.contains("diff.tool"),
        "{text}"
    );
}

#[test]
fn a_tool_git_does_not_know_fails_with_what_git_said() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "diff.tool", "no-such-tool-anywhere"])
        .unwrap();
    std::fs::write(f.path().join("file0.txt"), "on disk\n").unwrap();

    let err = open(&f)
        .launch_difftool(&DiffSpec::WorkTreeVsIndex, "file0.txt", None, None)
        .unwrap_err();

    let GitError::Command(failed) = err else {
        panic!("expected git's failure, got {err:?}");
    };
    assert!(
        failed.stderr.contains("no-such-tool-anywhere"),
        "{failed:?}"
    );
}

#[test]
fn a_terminal_tool_is_refused_before_it_could_hang_unseen() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "diff.tool", "vimdiff"]).unwrap();

    let err = open(&f)
        .launch_difftool(&DiffSpec::WorkTreeVsIndex, "file0.txt", None, None)
        .unwrap_err();

    assert!(matches!(err, GitError::InvalidState(text) if text.contains("terminal")));
}

#[test]
fn diff_tool_wins_over_merge_tool_and_the_last_value_counts() {
    let listing = "merge.tool\nmeld\0diff.tool\nkdiff3\0diff.tool\nbc\0";
    assert_eq!(diff_tool_from_listing(listing).as_deref(), Some("bc"));
    assert_eq!(
        diff_tool_from_listing("merge.tool\nmeld\0").as_deref(),
        Some("meld")
    );
    assert_eq!(
        diff_tool_from_listing("diff.tool\nmeld\0diff.tool\n\0"),
        None
    );
}

#[test]
fn a_rename_names_both_paths_and_a_program_of_cogit_is_its_own_tool() {
    let args = difftool_args(
        "meld",
        &["a".into(), "b".into()],
        false,
        "new.txt",
        Some("old.txt"),
        Some("x \"$LOCAL\""),
    );
    assert_eq!(
        args,
        [
            "-c",
            "difftool.cogit.cmd=x \"$LOCAL\"",
            "-c",
            "difftool.cogit.trustExitCode=false",
            "difftool",
            "--no-prompt",
            "--tool=cogit",
            "-M",
            "a",
            "b",
            "--",
            "old.txt",
            "new.txt"
        ]
    );
    let plain = difftool_args("meld", &[], true, "a.txt", None, None);
    assert_eq!(
        plain,
        [
            "difftool",
            "--no-prompt",
            "--tool=meld",
            "--cached",
            "--",
            "a.txt"
        ]
    );
}
