// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What a file summarised in Diff shows about itself: what `.gitattributes` says about
//! reading it as lines, and the object id of each side (R-531).

use git_engine::{DiffContent, DiffSpec, MAX_HASHED_BYTES, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn rev(f: &test_fixtures::Fixture, spec: &str) -> String {
    f.git(&["rev-parse", spec]).unwrap().trim().to_owned()
}

#[test]
fn the_attributes_say_how_a_diff_reads_each_path() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(
        ".gitattributes",
        "*.dat binary\n*.lock -diff\n*.txt text\n*.log diff\n*.auto text=auto\n*.crlf -text\n",
    )
    .unwrap();
    let repo = open(&f);
    let mut attributes = repo.diff_attributes();

    for (path, expected) in [
        ("table.dat", DiffContent::Binary("binary")),
        ("deps.lock", DiffContent::Binary("-diff")),
        ("notes.txt", DiffContent::Text),
        ("run.log", DiffContent::Text),
        ("x.auto", DiffContent::Detect),
        ("x.crlf", DiffContent::Detect),
        ("plain.md", DiffContent::Detect),
    ] {
        assert_eq!(attributes.content(path), expected, "{path}");
    }
}

#[test]
fn each_side_of_a_commit_is_named_by_its_blob() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(1, "file0.txt", "rewritten\n").unwrap();
    let repo = open(&f);
    let spec = DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    };

    let ids = repo.side_ids_from(&spec, "file0.txt", "file0.txt").unwrap();

    assert_eq!(
        ids,
        (
            Some(rev(&f, "HEAD^:file0.txt")),
            Some(rev(&f, "HEAD:file0.txt"))
        )
    );
}

#[test]
fn a_working_file_is_named_as_hash_object_names_it() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.txt text eol=lf\n")
        .unwrap();
    f.git(&["add", "--", ".gitattributes"]).unwrap();
    f.commit_staged(1, "attributes").unwrap();
    // The clean filter turns CRLF into LF: the id is that of what `git add` would store.
    f.write_file("file0.txt", "one\r\ntwo\r\n").unwrap();
    f.write_file("added.txt", "new\n").unwrap();
    let repo = open(&f);
    let hash = |path: &str| {
        f.git(&["hash-object", "--", path])
            .unwrap()
            .trim()
            .to_owned()
    };

    let changed = repo
        .side_ids_from(&DiffSpec::WorkTreeVsIndex, "file0.txt", "file0.txt")
        .unwrap();
    let added = repo
        .side_ids_from(&DiffSpec::WorkTreeVsIndex, "added.txt", "added.txt")
        .unwrap();

    assert_eq!(
        changed,
        (Some(rev(&f, ":file0.txt")), Some(hash("file0.txt")))
    );
    assert_eq!(added, (None, Some(hash("added.txt"))));
}

#[test]
fn a_working_file_too_large_to_read_goes_unnamed() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::File::create(f.path().join("dump.bin"))
        .unwrap()
        .set_len(MAX_HASHED_BYTES + 1)
        .unwrap();
    let repo = open(&f);

    let ids = repo
        .side_ids_from(&DiffSpec::WorkTreeVsIndex, "dump.bin", "dump.bin")
        .unwrap();

    assert_eq!(ids, (None, None));
}
