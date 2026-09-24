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
