// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the client reads through gix, against what the `git` command prints for the same
//! repository. A difference here is a bug in the reading, not in the test.

use git_engine::RepoHandle;
use test_fixtures::Fixture;

fn open(f: &Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

mod stash {
    use super::*;

    fn ours(f: &Fixture) -> Vec<String> {
        open(f)
            .stashes()
            .unwrap()
            .into_iter()
            .map(|entry| format!("stash@{{{}}} {} {}", entry.index, entry.oid, entry.message))
            .collect()
    }

    fn git(f: &Fixture) -> Vec<String> {
        lines(&f.git(&["stash", "list", "--format=%gd %H %gs"]).unwrap())
    }

    fn stash(f: &Fixture, index: i64, file: &str, args: &[&str]) {
        f.write_file(file, &format!("work {index}\n")).unwrap();
        let mut all = vec!["stash", "push", "--include-untracked"];
        all.extend_from_slice(args);
        f.git_at(index, &all).unwrap();
    }

    #[test]
    fn no_stash_lists_nothing_either_way() {
        let f = test_fixtures::linear(2).unwrap();
        assert!(git(&f).is_empty());
        assert_eq!(ours(&f), git(&f));
    }

    #[test]
    fn messages_order_and_indexes_match_stash_list() {
        let f = test_fixtures::linear(2).unwrap();
        stash(&f, 10, "a.txt", &["-m", "with a message"]);
        stash(&f, 11, "b.txt", &[]);
        f.git(&["checkout", "--detach", "HEAD~1"]).unwrap();
        stash(&f, 12, "c.txt", &[]);
        stash(&f, 13, "d.txt", &["-m", "detached, named"]);
        f.git(&["checkout", "main"]).unwrap();
        f.write_file("file0.txt", "kept in the tree\n").unwrap();
        let created = f.git(&["stash", "create", "made by create"]).unwrap();
        f.git(&["stash", "store", "-m", "stored: by hand", created.trim()])
            .unwrap();

        let expected = git(&f);
        assert_eq!(expected.len(), 5, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn dropping_one_renumbers_the_rest_the_same_way() {
        let f = test_fixtures::with_stashes(4).unwrap();
        f.git(&["stash", "drop", "stash@{1}"]).unwrap();
        f.git(&["stash", "pop"]).unwrap();

        let expected = git(&f);
        assert_eq!(expected.len(), 2, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }
}

mod staged {
    use super::*;
    use git_engine::FileStatus;

    /// `diff --cached --name-status` lines, the rename score dropped: the client keeps none.
    fn git(f: &Fixture, renames: &str) -> Vec<String> {
        let text = f
            .git(&["diff", "--cached", "--name-status", renames])
            .unwrap();
        let mut out: Vec<String> = text
            .lines()
            .map(|line| {
                let (status, rest) = line.split_once('\t').unwrap();
                // The client has no type change: a file turned symlink is `Modified` with
                // the new mode, which is what the panel draws.
                let letter = if status == "T" { "M" } else { &status[..1] };
                format!("{letter}\t{rest}")
            })
            .collect();
        out.sort();
        out
    }

    fn ours(f: &Fixture) -> Vec<String> {
        let files = open(f).worktree_files().unwrap();
        let mut out: Vec<String> = files
            .staged
            .iter()
            .map(|file| {
                let letter = match file.status {
                    FileStatus::Added => "A",
                    FileStatus::Modified => "M",
                    FileStatus::Deleted => "D",
                    FileStatus::Renamed => "R",
                    FileStatus::Copied => "C",
                    other => panic!("{other:?} in the staged list"),
                };
                match &file.old_path {
                    Some(old) => format!("{letter}\t{old}\t{}", file.path),
                    None => format!("{letter}\t{}", file.path),
                }
            })
            .collect();
        // Git lists an unmerged path as `U`; the client keeps it with the unstaged files.
        out.extend(
            files
                .unstaged
                .iter()
                .filter(|file| file.status == FileStatus::Conflicted)
                .map(|file| format!("U\t{}", file.path)),
        );
        out.sort();
        out
    }

    #[test]
    fn additions_deletions_edits_and_modes_match_diff_cached() {
        let f = test_fixtures::linear(4).unwrap();
        f.write_file("new/added.txt", "fresh\n").unwrap();
        f.git(&["add", "--", "new/added.txt"]).unwrap();
        f.git(&["rm", "-q", "--", "file0.txt"]).unwrap();
        f.write_file("file1.txt", "edited\n").unwrap();
        f.git(&["add", "--", "file1.txt"]).unwrap();
        f.git(&["update-index", "--chmod=+x", "file2.txt"]).unwrap();
        // Unstaged and intent-to-add changes are not in `diff --cached`.
        f.write_file("file3.txt", "not staged\n").unwrap();
        f.write_file("later.txt", "intent only\n").unwrap();
        f.git(&["add", "-N", "--", "later.txt"]).unwrap();

        let expected = git(&f, "--no-renames");
        assert_eq!(expected.len(), 4, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn nothing_staged_lists_nothing() {
        let f = test_fixtures::linear(2).unwrap();
        f.write_file("file0.txt", "only in the tree\n").unwrap();
        assert!(git(&f, "--no-renames").is_empty());
        assert_eq!(ours(&f), git(&f, "--no-renames"));
    }

    #[test]
    fn an_unborn_head_compares_against_nothing() {
        let f = test_fixtures::empty().unwrap();
        f.write_file("first.txt", "one\n").unwrap();
        f.write_file("dir/second.txt", "two\n").unwrap();
        f.git(&["add", "--", "."]).unwrap();

        let expected = git(&f, "--no-renames");
        assert_eq!(expected.len(), 2, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn a_moved_and_an_added_submodule_match_diff_cached() {
        let f = test_fixtures::with_submodule().unwrap();
        let module = f.path().join("vendor/lib");
        f.git_in(&module, &["commit", "-q", "--allow-empty", "-m", "moved"])
            .unwrap();
        f.git(&["add", "--", "vendor/lib"]).unwrap();
        let head = f.oid("HEAD").unwrap();
        f.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},vendor/other"),
        ])
        .unwrap();

        let expected = git(&f, "--no-renames");
        assert_eq!(expected.len(), 2, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn a_conflict_is_reported_once_as_unmerged() {
        let f = test_fixtures::conflicted().unwrap();
        let expected = git(&f, "--no-renames");
        assert_eq!(expected, vec!["U\tconflict.txt".to_owned()]);
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn a_plain_rename_matches_diff_cached_m() {
        let f = test_fixtures::linear(1).unwrap();
        let body = (0..20).map(|i| format!("line {i}\n")).collect::<String>();
        f.commit_file(1, "old-name.txt", &body).unwrap();
        f.git(&["mv", "old-name.txt", "new-name.txt"]).unwrap();

        let expected = git(&f, "-M");
        assert_eq!(expected, vec!["R\told-name.txt\tnew-name.txt".to_owned()]);
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn a_file_turned_symlink_in_the_index_matches_diff_cached() {
        let f = test_fixtures::linear(2).unwrap();
        let target = f.git(&["hash-object", "-w", "--", "file0.txt"]).unwrap();
        f.git(&[
            "update-index",
            "--cacheinfo",
            &format!("120000,{},file1.txt", target.trim()),
        ])
        .unwrap();

        let expected = git(&f, "--no-renames");
        assert_eq!(expected.len(), 1, "{expected:?}");
        assert_eq!(ours(&f), expected);
    }

    #[test]
    fn of_two_identical_sources_the_one_with_the_same_name_is_the_rename() {
        let f = test_fixtures::linear(1).unwrap();
        let body = (0..20).map(|i| format!("line {i}\n")).collect::<String>();
        f.write_file("x/first.txt", &body).unwrap();
        f.write_file("y/moved.txt", &body).unwrap();
        f.git(&["add", "--", "."]).unwrap();
        f.commit_staged(1, "two copies").unwrap();
        f.git(&["rm", "-q", "--", "x/first.txt", "y/moved.txt"])
            .unwrap();
        f.write_file("z/moved.txt", &body).unwrap();
        f.git(&["add", "--", "z/moved.txt"]).unwrap();

        let expected = git(&f, "-M");
        assert_eq!(
            expected,
            vec![
                "D\tx/first.txt".to_owned(),
                "R\ty/moved.txt\tz/moved.txt".to_owned()
            ]
        );
        assert_eq!(ours(&f), expected);
    }
}
