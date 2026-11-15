// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Repository ▸ Sparse Checkout (F-955).

use git_engine::{GitError, RepoHandle, SparseChange, SparseFolder, SparseState};

/// `top.txt`, `a/x.txt`, `a/deep/y.txt`, `b/z.txt`, `c/w.txt`: five files, three folders.
fn folders() -> test_fixtures::Fixture {
    let f = test_fixtures::empty().unwrap();
    for name in ["top.txt", "a/x.txt", "a/deep/y.txt", "b/z.txt", "c/w.txt"] {
        f.write_file(name, &format!("{name}\n")).unwrap();
    }
    f.git(&["add", "--all"]).unwrap();
    f.commit_staged(1, "folders").unwrap();
    f
}

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn cone(set: &[&str]) -> SparseChange {
    SparseChange::Set {
        cone: true,
        set: set.iter().map(|each| (*each).to_owned()).collect(),
    }
}

fn here(f: &test_fixtures::Fixture, name: &str) -> bool {
    f.path().join(name).exists()
}

#[test]
fn a_full_checkout_says_sparse_is_off_and_offers_cone() {
    let f = folders();
    assert_eq!(
        open(&f).sparse_state().unwrap(),
        SparseState {
            enabled: false,
            cone: true,
            set: Vec::new()
        }
    );
}

#[test]
fn folders_of_head_are_listed_one_level_at_a_time() {
    let f = folders();
    let repo = open(&f);
    let folder = |path: &str, folders: bool| SparseFolder {
        path: path.to_owned(),
        folders,
    };

    assert_eq!(
        repo.sparse_folders("").unwrap(),
        vec![folder("a", true), folder("b", false), folder("c", false)]
    );
    assert_eq!(
        repo.sparse_folders("a").unwrap(),
        vec![folder("a/deep", false)]
    );
    assert!(repo.sparse_folders("missing").unwrap().is_empty());
}

#[test]
fn enabling_cone_with_two_folders_keeps_them_and_the_top_files() {
    let f = folders();
    let repo = open(&f);

    let preview = repo.sparse_preview(&cone(&["a", "c"])).unwrap();
    assert_eq!(
        (
            preview.tracked,
            preview.present,
            preview.removed,
            preview.appearing
        ),
        (5, 4, 1, 0)
    );
    assert!(preview.blocked.is_empty());

    repo.sparse_apply(&cone(&["a", "c"])).unwrap();

    assert!(here(&f, "top.txt") && here(&f, "a/deep/y.txt") && here(&f, "c/w.txt"));
    assert!(!here(&f, "b/z.txt"));
    assert_eq!(
        repo.sparse_state().unwrap(),
        SparseState {
            enabled: true,
            cone: true,
            set: vec!["a".to_owned(), "c".to_owned()]
        }
    );
    assert_eq!(open(&f).sparse_size(), Some(2));
    assert_eq!(git_engine::pulse(f.path()).sparse, Some(2));
}

#[test]
fn changing_the_set_counts_what_comes_and_goes() {
    let f = folders();
    let repo = open(&f);
    repo.sparse_apply(&cone(&["a"])).unwrap();

    let preview = repo.sparse_preview(&cone(&["b"])).unwrap();
    assert_eq!(
        (preview.present, preview.removed, preview.appearing),
        (2, 2, 1)
    );

    repo.sparse_apply(&cone(&["b"])).unwrap();
    assert!(here(&f, "b/z.txt") && !here(&f, "a/x.txt"));
}

#[test]
fn a_leaving_file_with_local_changes_refuses_and_changes_nothing() {
    let f = folders();
    let repo = open(&f);
    f.write_file("b/z.txt", "edited\n").unwrap();

    let preview = repo.sparse_preview(&cone(&["a"])).unwrap();
    assert_eq!(preview.blocked, vec!["b/z.txt".to_owned()]);

    let err = repo.sparse_apply(&cone(&["a"])).unwrap_err();
    assert!(
        matches!(&err, GitError::InvalidState(text) if text.contains("b/z.txt")),
        "{err:?}"
    );
    assert!(!repo.sparse_state().unwrap().enabled);
    assert!(here(&f, "c/w.txt"));
    assert_eq!(
        std::fs::read_to_string(f.path().join("b/z.txt")).unwrap(),
        "edited\n"
    );
}

#[test]
fn disabling_brings_every_file_back() {
    let f = folders();
    let repo = open(&f);
    repo.sparse_apply(&cone(&[])).unwrap();
    assert!(!here(&f, "a/x.txt"));

    let preview = repo.sparse_preview(&SparseChange::Disable).unwrap();
    assert_eq!(
        (preview.present, preview.appearing, preview.removed),
        (5, 4, 0)
    );

    repo.sparse_apply(&SparseChange::Disable).unwrap();
    for name in ["top.txt", "a/x.txt", "a/deep/y.txt", "b/z.txt", "c/w.txt"] {
        assert!(here(&f, name), "{name}");
    }
    assert!(!repo.sparse_state().unwrap().enabled);
}

#[test]
fn a_non_cone_repository_shows_its_patterns() {
    let f = folders();
    f.git(&["sparse-checkout", "set", "--no-cone", "/*.txt", "/b/"])
        .unwrap();
    let repo = open(&f);

    assert_eq!(
        repo.sparse_state().unwrap(),
        SparseState {
            enabled: true,
            cone: false,
            set: vec!["/*.txt".to_owned(), "/b/".to_owned()]
        }
    );
    let preview = repo
        .sparse_preview(&SparseChange::Set {
            cone: false,
            set: vec!["/*.txt".to_owned(), "/c/".to_owned()],
        })
        .unwrap();
    assert_eq!(
        (preview.present, preview.removed, preview.appearing),
        (2, 1, 1)
    );
}
