// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::SharedRepo;

#[test]
fn a_handle_from_the_shared_repository_reads_like_a_fresh_one() {
    let f = test_fixtures::with_remote().unwrap();
    let shared = SharedRepo::open(f.path()).unwrap();

    let handle = shared.handle().unwrap();

    assert_eq!(handle.root(), f.path());
    assert_eq!(handle.remotes().unwrap(), vec!["origin".to_owned()]);
    assert!(shared.is_current());
}

// The config is a snapshot taken at open; `git config` from anywhere makes it stale.
// The previous iteration read the old flow config right after `flow_init` wrote it.
#[test]
fn a_config_written_by_git_makes_it_stale() {
    let f = test_fixtures::linear(2).unwrap();
    let shared = SharedRepo::open(f.path()).unwrap();
    assert!(shared.handle().unwrap().remotes().unwrap().is_empty());

    f.git(&["remote", "add", "upstream", "https://example.invalid/x.git"])
        .unwrap();

    assert!(!shared.is_current());
    let reopened = SharedRepo::open(f.path()).unwrap();
    assert_eq!(
        reopened.handle().unwrap().remotes().unwrap(),
        vec!["upstream".to_owned()]
    );
}

#[test]
fn refs_and_objects_written_by_git_are_seen_without_reopening() {
    let f = test_fixtures::linear(2).unwrap();
    let shared = SharedRepo::open(f.path()).unwrap();
    let before = shared.handle().unwrap().head().unwrap();

    f.commit_file(30, "later.txt", "later\n").unwrap();
    f.git(&["branch", "topic"]).unwrap();

    assert!(shared.is_current());
    let handle = shared.handle().unwrap();
    assert_ne!(handle.head().unwrap(), before);
    assert!(handle.branches().unwrap().iter().any(|b| b.name == "topic"));
}

#[test]
fn a_repository_that_went_away_is_stale() {
    let f = test_fixtures::linear(1).unwrap();
    let shared = SharedRepo::open(f.path()).unwrap();

    std::fs::remove_dir_all(f.path().join(".git")).unwrap();

    assert!(!shared.is_current());
}

// Every handle of one SharedRepo shares gix's index snapshot, reread only on a strictly
// newer mtime. Two stages within one tick of a coarse clock (FAT32 keeps 2 s) left the
// second file unstaged in every status read after them.
#[test]
fn an_index_rewritten_within_the_same_mtime_is_read_again() {
    let f = test_fixtures::linear(1).unwrap();
    let shared = SharedRepo::open(f.path()).unwrap();
    f.write_file("a.txt", "a\n").unwrap();
    f.write_file("b.txt", "b\n").unwrap();
    f.git(&["add", "--", "a.txt"]).unwrap();
    let index = f.git_dir().join("index");
    let first = std::fs::metadata(&index).unwrap().modified().unwrap();
    shared.handle().unwrap().worktree_files().unwrap();

    f.git(&["add", "--", "b.txt"]).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&index)
        .unwrap()
        .set_modified(first)
        .unwrap();

    let files = shared.handle().unwrap().worktree_files().unwrap();
    let staged: Vec<&str> = files.staged.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(staged, ["a.txt", "b.txt"]);
}

/// Big enough that gix would map `packed-refs` rather than read it (32 KiB).
fn many_packed_branches(f: &test_fixtures::Fixture) {
    let head = f.oid("HEAD").unwrap();
    let mut packed = String::from("# pack-refs with: peeled fully-peeled sorted \n");
    for n in 0..800 {
        packed.push_str(&format!(
            "{head} refs/heads/some-longer-branch-name-{n:04}\n"
        ));
    }
    std::fs::write(f.path().join(".git/packed-refs"), packed).unwrap();
}

// Windows refuses to replace a file that is mapped into a process: a `packed-refs` kept
// mapped by the cached repository would make every `git branch -d` of a packed branch
// and every `pack-refs` fail while Cogit has the repository open.
#[test]
fn git_can_rewrite_packed_refs_while_the_repository_is_shared() {
    let f = test_fixtures::linear(1).unwrap();
    many_packed_branches(&f);
    let shared = SharedRepo::open(f.path()).unwrap();
    let listed = shared.handle().unwrap().branches().unwrap().len();
    assert!(listed > 800);

    f.git(&["branch", "-D", "some-longer-branch-name-0001"])
        .unwrap();

    assert_eq!(
        shared.handle().unwrap().branches().unwrap().len(),
        listed - 1
    );
}

// The same for packs: `gc` and `repack` delete the old ones, which a mapping would pin.
#[test]
fn git_can_delete_packs_while_the_repository_is_shared() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["gc", "-q"]).unwrap();
    let pack_dir = f.path().join(".git/objects/pack");
    let packs = |dir: &std::path::Path| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "pack"))
            .collect()
    };
    let old = packs(&pack_dir);
    let shared = SharedRepo::open(f.path()).unwrap();
    let handle = shared.handle().unwrap();
    handle.commit_details("HEAD~1").unwrap();
    drop(handle);

    f.commit_file(40, "more.txt", "more\n").unwrap();
    f.git(&["repack", "-a", "-d", "-q"]).unwrap();

    assert!(old.iter().all(|pack| !pack.exists()), "{old:?} still there");
    assert!(shared.handle().unwrap().commit_details("HEAD~1").is_ok());
}
