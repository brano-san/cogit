// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The operations the graph and Branches context menus need beyond what the toolbar had.

use git_engine::{RepoHandle, ResetMode, TagRequest};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn annotated(repo: &RepoHandle, name: &str, message: &str) {
    repo.create_tag(&TagRequest {
        name: name.to_owned(),
        target: None,
        message: Some(message.to_owned()),
        force: false,
    })
    .unwrap();
}

fn lightweight(repo: &RepoHandle, name: &str) {
    repo.create_tag(&TagRequest {
        name: name.to_owned(),
        target: None,
        message: None,
        force: false,
    })
    .unwrap();
}

mod reset {
    use super::*;

    #[test]
    fn soft_moves_the_branch_and_keeps_the_changes_staged() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        let target = f.oid("HEAD~1").unwrap();

        repo.reset(&target, ResetMode::Soft).unwrap();

        assert_eq!(f.oid("HEAD").unwrap(), target);
        assert_eq!(
            f.oid("main").unwrap(),
            target,
            "the branch moves, not HEAD alone"
        );
        assert_eq!(repo.status().unwrap().staged, 1);
    }

    #[test]
    fn mixed_keeps_the_changes_in_the_working_tree_only() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        let target = f.oid("HEAD~1").unwrap();

        repo.reset(&target, ResetMode::Mixed).unwrap();

        let status = repo.status().unwrap();
        assert_eq!(f.oid("HEAD").unwrap(), target);
        assert_eq!(status.staged, 0);
        assert_eq!(
            status.untracked, 1,
            "the file the dropped commit added is left behind"
        );
    }

    #[test]
    fn hard_throws_the_changes_away() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        let target = f.oid("HEAD~1").unwrap();

        repo.reset(&target, ResetMode::Hard).unwrap();

        assert_eq!(f.oid("HEAD").unwrap(), target);
        assert!(repo.status().unwrap().is_clean());
        assert!(!f.path().join("file2.txt").exists());
    }

    #[test]
    fn keep_carries_an_unrelated_local_edit_across() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        f.write_file("file0.txt", "edited locally\n").unwrap();
        let target = f.oid("HEAD~1").unwrap();

        repo.reset(&target, ResetMode::Keep).unwrap();

        assert_eq!(f.oid("HEAD").unwrap(), target);
        assert!(!f.path().join("file2.txt").exists());
        let kept = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
        assert_eq!(kept, "edited locally\n");
    }

    #[test]
    fn keep_refuses_to_overwrite_a_local_edit_the_reset_would_touch() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        f.write_file("file2.txt", "edited locally\n").unwrap();
        let before = f.oid("HEAD").unwrap();

        let result = repo.reset(&f.oid("HEAD~1").unwrap(), ResetMode::Keep);

        assert!(result.is_err());
        assert_eq!(f.oid("HEAD").unwrap(), before, "nothing moved");
    }

    #[test]
    fn merge_moves_the_branch_and_keeps_an_unrelated_edit() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        f.write_file("file0.txt", "edited locally\n").unwrap();
        let target = f.oid("HEAD~1").unwrap();

        repo.reset(&target, ResetMode::Merge).unwrap();

        assert_eq!(f.oid("HEAD").unwrap(), target);
        let kept = std::fs::read_to_string(f.path().join("file0.txt")).unwrap();
        assert_eq!(kept, "edited locally\n");
    }

    #[test]
    fn an_unknown_revision_is_an_error() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        assert!(repo.reset("no-such-rev", ResetMode::Mixed).is_err());
    }
}

mod ancestry {
    use super::*;

    #[test]
    fn an_older_commit_of_the_branch_is_an_ancestor_of_head() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        assert!(repo.is_ancestor(&f.oid("HEAD~2").unwrap(), "HEAD").unwrap());
        assert!(
            repo.is_ancestor("HEAD", "HEAD").unwrap(),
            "a commit counts as its own"
        );
    }

    #[test]
    fn a_descendant_is_not_an_ancestor() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        assert!(!repo.is_ancestor("HEAD", &f.oid("HEAD~2").unwrap()).unwrap());
    }

    #[test]
    fn a_commit_of_a_side_branch_is_not_on_head() {
        let f = test_fixtures::branched().unwrap();
        let repo = open(&f);
        let side = repo
            .branches()
            .unwrap()
            .into_iter()
            .find(|b| !b.is_head && b.kind == git_engine::BranchKind::Local)
            .unwrap();
        assert!(!repo.is_ancestor(&side.oid, "HEAD").unwrap());
    }
}

mod compare {
    use super::*;

    #[test]
    fn lists_every_file_that_differs_between_two_commits() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);

        let files = repo
            .files_between(&f.oid("HEAD~2").unwrap(), "HEAD")
            .unwrap();

        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["file1.txt", "file2.txt"]);
        assert!(
            files
                .iter()
                .all(|file| file.status == git_engine::FileStatus::Added)
        );
    }

    #[test]
    fn the_other_direction_deletes_them() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);

        let files = repo
            .files_between("HEAD", &f.oid("HEAD~2").unwrap())
            .unwrap();

        assert!(
            files
                .iter()
                .all(|file| file.status == git_engine::FileStatus::Deleted)
        );
    }

    #[test]
    fn a_commit_against_itself_differs_in_nothing() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        assert!(repo.files_between("HEAD", "HEAD").unwrap().is_empty());
    }
}

mod tag_names {
    use super::*;

    #[test]
    fn a_fresh_valid_name_has_no_problem() {
        let f = test_fixtures::linear(1).unwrap();
        assert_eq!(open(&f).tag_name_problem("v1.0").unwrap(), None);
    }

    #[test]
    fn a_name_git_rejects_is_reported() {
        let f = test_fixtures::linear(1).unwrap();
        let repo = open(&f);
        for bad in ["two..dots", "ends.lock", "has space", "trailing/"] {
            let problem = repo.tag_name_problem(bad).unwrap();
            assert!(
                problem
                    .as_deref()
                    .is_some_and(|text| text.contains("not a valid")),
                "{bad}: {problem:?}"
            );
        }
    }

    #[test]
    fn a_taken_name_is_reported() {
        let f = test_fixtures::linear(1).unwrap();
        let repo = open(&f);
        lightweight(&repo, "v1.0");

        let problem = repo.tag_name_problem("v1.0").unwrap();

        assert!(problem.is_some_and(|text| text.contains("already exists")));
    }

    #[test]
    fn an_empty_name_is_reported() {
        let f = test_fixtures::linear(1).unwrap();
        assert!(open(&f).tag_name_problem("  ").unwrap().is_some());
    }
}

mod tag_messages {
    use super::*;

    #[test]
    fn an_annotated_tag_gives_its_message() {
        let f = test_fixtures::linear(1).unwrap();
        let repo = open(&f);
        annotated(&repo, "v1.0", "First release\n\nWith notes.");

        assert_eq!(
            repo.tag_message("v1.0").unwrap().as_deref(),
            Some("First release\n\nWith notes.")
        );
    }

    #[test]
    fn a_lightweight_tag_has_none() {
        let f = test_fixtures::linear(1).unwrap();
        let repo = open(&f);
        lightweight(&repo, "v1.0");
        assert_eq!(repo.tag_message("v1.0").unwrap(), None);
    }

    #[test]
    fn an_unknown_tag_is_an_error() {
        let f = test_fixtures::linear(1).unwrap();
        assert!(open(&f).tag_message("nope").is_err());
    }
}

mod tag_rename {
    use super::*;

    fn tag(repo: &RepoHandle, name: &str) -> Option<git_engine::Tag> {
        repo.tags().unwrap().into_iter().find(|t| t.name == name)
    }

    #[test]
    fn a_lightweight_tag_keeps_its_commit() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        lightweight(&repo, "old");

        repo.rename_tag("old", "new").unwrap();

        assert!(tag(&repo, "old").is_none());
        let renamed = tag(&repo, "new").unwrap();
        assert_eq!(renamed.oid, f.oid("HEAD").unwrap());
        assert!(!renamed.is_annotated);
    }

    #[test]
    fn an_annotated_tag_stays_annotated_with_its_message() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        annotated(&repo, "old", "Release notes");

        repo.rename_tag("old", "new").unwrap();

        assert!(tag(&repo, "old").is_none());
        assert!(tag(&repo, "new").unwrap().is_annotated);
        assert_eq!(
            repo.tag_message("new").unwrap().as_deref(),
            Some("Release notes")
        );
    }

    #[test]
    fn a_taken_name_leaves_both_tags_alone() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        lightweight(&repo, "old");
        lightweight(&repo, "new");

        assert!(repo.rename_tag("old", "new").is_err());
        assert!(tag(&repo, "old").is_some());
    }
}

mod stash_rename {
    use super::*;

    fn listing(repo: &RepoHandle) -> Vec<(String, String)> {
        repo.stashes()
            .unwrap()
            .into_iter()
            .map(|entry| (entry.oid, entry.message))
            .collect()
    }

    #[test]
    fn renames_an_entry_in_the_middle_without_moving_it() {
        let f = test_fixtures::with_stashes(3).unwrap();
        let repo = open(&f);
        let before = listing(&repo);

        repo.rename_stash(1, "renamed").unwrap();

        let after = listing(&repo);
        assert_eq!(after.len(), 3);
        let oids = |list: &[(String, String)]| list.iter().map(|e| e.0.clone()).collect::<Vec<_>>();
        assert_eq!(oids(&after), oids(&before), "the order is kept");
        assert_eq!(after[1].1, "renamed");
        assert_eq!(
            after[0].1, before[0].1,
            "the entries above keep their messages"
        );
        assert_eq!(after[2].1, before[2].1);
    }

    #[test]
    fn renames_the_newest_entry() {
        let f = test_fixtures::with_stashes(2).unwrap();
        let repo = open(&f);
        let before = listing(&repo);

        repo.rename_stash(0, "top").unwrap();

        let after = listing(&repo);
        assert_eq!(after[0], (before[0].0.clone(), "top".to_owned()));
        assert_eq!(after[1], before[1]);
    }

    #[test]
    fn an_index_out_of_range_changes_nothing() {
        let f = test_fixtures::with_stashes(2).unwrap();
        let repo = open(&f);
        let before = listing(&repo);

        assert!(repo.rename_stash(5, "nope").is_err());
        assert_eq!(listing(&repo), before);
    }

    #[test]
    fn an_empty_message_is_refused() {
        let f = test_fixtures::with_stashes(1).unwrap();
        let repo = open(&f);
        let before = listing(&repo);

        assert!(repo.rename_stash(0, "   ").is_err());
        assert_eq!(listing(&repo), before);
    }
}

mod author {
    use super::*;

    fn author_of(repo: &RepoHandle, rev: &str) -> (String, String) {
        let details = repo.commit_details(rev).unwrap();
        (details.author.name, details.author.email)
    }

    #[test]
    fn rewrites_the_author_of_an_older_commit_only() {
        let f = test_fixtures::linear(3).unwrap();
        let repo = open(&f);
        let root = f.oid("HEAD~2").unwrap();

        repo.edit_author(&f.oid("HEAD~1").unwrap(), "New Name", "new@cogit.test")
            .unwrap();

        assert_eq!(
            author_of(&repo, "HEAD~1"),
            ("New Name".to_owned(), "new@cogit.test".to_owned())
        );
        assert_eq!(author_of(&repo, "HEAD").0, test_fixtures::AUTHOR_NAME);
        assert_eq!(
            f.oid("HEAD~2").unwrap(),
            root,
            "the commits before it are untouched"
        );
        assert_eq!(repo.commit_details("HEAD~1").unwrap().summary, "commit 1");
        assert_eq!(repo.commit_details("HEAD").unwrap().summary, "commit 2");
    }

    #[test]
    fn rewrites_the_head_commit() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);

        repo.edit_author("HEAD", "Top", "top@cogit.test").unwrap();

        assert_eq!(author_of(&repo, "HEAD").0, "Top");
        assert_eq!(author_of(&repo, "HEAD~1").0, test_fixtures::AUTHOR_NAME);
    }

    #[test]
    fn rewrites_the_root_commit() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);

        repo.edit_author(&f.oid("HEAD~1").unwrap(), "Root", "root@cogit.test")
            .unwrap();

        assert_eq!(author_of(&repo, "HEAD~1").0, "Root");
        assert_eq!(author_of(&repo, "HEAD").0, test_fixtures::AUTHOR_NAME);
    }

    #[test]
    fn a_dirty_working_tree_is_refused() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        f.write_file("file0.txt", "dirty\n").unwrap();
        let before = f.oid("HEAD").unwrap();

        assert!(repo.edit_author("HEAD", "X", "x@cogit.test").is_err());
        assert_eq!(f.oid("HEAD").unwrap(), before);
    }

    #[test]
    fn an_empty_or_malformed_identity_is_refused() {
        let f = test_fixtures::linear(2).unwrap();
        let repo = open(&f);
        assert!(repo.edit_author("HEAD", "", "x@cogit.test").is_err());
        assert!(repo.edit_author("HEAD", "X", "").is_err());
        assert!(repo.edit_author("HEAD", "X <y>", "x@cogit.test").is_err());
    }
}

mod root_plan {
    use super::*;

    #[test]
    fn a_plan_from_the_root_lists_every_commit() {
        let f = test_fixtures::linear(3).unwrap();
        let plan = open(&f).rebase_todo("--root").unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].oid, f.oid("HEAD~2").unwrap());
    }
}
