// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `is_published` and `protecting_refs` against `git for-each-ref --contains` itself, for
//! every commit of a repository with merges, two remotes, a symbolic `origin/HEAD` and
//! commits the commit-graph does not have — with the commit-graph and without it.

use git_engine::RepoHandle;
use test_fixtures::Fixture;

struct Setup {
    f: Fixture,
    _remotes: tempfile::TempDir,
}

fn bare(remotes: &tempfile::TempDir, f: &Fixture, name: &str) -> String {
    let path = remotes.path().join(format!("{name}.git"));
    let path = path.to_string_lossy().replace('\\', "/");
    f.git(&["init", "-q", "--bare", &path]).unwrap();
    f.git(&["remote", "add", name, &path]).unwrap();
    path
}

/// main: c0 c1 c2 · side from c1: s1 s2 · merged: c2 + side (--no-ff), pushed as
/// `origin/release/1.0`, so s1 and s2 are reachable only through a second parent.
/// `upstream` has main and `develop` = s1. After the commit-graph is written: `late` on
/// origin moves past the merge, and main and `local-side` get commits nobody has.
fn setup(with_graph: bool) -> Setup {
    let f = test_fixtures::linear(3).unwrap();
    let remotes = tempfile::TempDir::new().unwrap();
    bare(&remotes, &f, "origin");
    bare(&remotes, &f, "upstream");
    f.git(&["config", "cogit.protectedBranches", "**"]).unwrap();

    f.git(&["checkout", "-q", "-b", "side", "HEAD~1"]).unwrap();
    f.commit_file(20, "side-1.txt", "s1\n").unwrap();
    f.commit_file(21, "side-2.txt", "s2\n").unwrap();
    f.git(&["checkout", "-q", "-b", "merged", "main"]).unwrap();
    f.merge(22, &["side"], "merge side").unwrap();

    f.git(&[
        "push",
        "-q",
        "origin",
        "main",
        "merged:refs/heads/release/1.0",
    ])
    .unwrap();
    f.git(&[
        "push",
        "-q",
        "upstream",
        "main",
        "side~1:refs/heads/develop",
    ])
    .unwrap();
    f.git(&["fetch", "-q", "--all"]).unwrap();
    f.git(&["remote", "set-head", "origin", "main"]).unwrap();

    if with_graph {
        f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    }

    f.commit_file(30, "late.txt", "late\n").unwrap();
    f.git(&["push", "-q", "origin", "HEAD:refs/heads/late"])
        .unwrap();
    f.git(&["fetch", "-q", "origin"]).unwrap();
    // A local branch and a tag named like remote refs: git then prints those refs as
    // `remotes/origin/late` and `remotes/upstream/main`.
    f.git(&["branch", "origin/late", "main"]).unwrap();
    f.git(&["tag", "upstream/main", "main"]).unwrap();
    f.git(&["checkout", "-q", "main"]).unwrap();
    f.commit_file(31, "local.txt", "local\n").unwrap();
    f.git(&["checkout", "-q", "-b", "local-side", "side~1"])
        .unwrap();
    f.commit_file(32, "local-side.txt", "local side\n").unwrap();

    assert_eq!(
        f.git_dir().join("objects/info/commit-graph").exists(),
        with_graph
    );
    Setup {
        f,
        _remotes: remotes,
    }
}

fn containing(f: &Fixture, oid: &str) -> Vec<String> {
    f.git(&[
        "for-each-ref",
        "--format=%(refname:short)",
        "--contains",
        oid,
        "refs/remotes",
    ])
    .unwrap()
    .lines()
    .map(str::to_owned)
    .collect()
}

fn every_commit(f: &Fixture) -> Vec<String> {
    f.git(&["rev-list", "--all"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn check(with_graph: bool) {
    let setup = setup(with_graph);
    let f = &setup.f;
    let repo = RepoHandle::open(f.path()).unwrap();
    let commits = every_commit(f);
    assert_eq!(commits.len(), 9, "{commits:?}");

    let mut published = 0;
    for oid in &commits {
        let expected = containing(f, oid);
        assert_eq!(
            repo.is_published(oid).unwrap(),
            !expected.is_empty(),
            "is_published {oid}, git says {expected:?}"
        );
        // `**` protects every branch, so this is the containing list itself.
        assert_eq!(
            repo.protecting_refs(oid).unwrap(),
            expected,
            "protecting_refs {oid}"
        );
        published += usize::from(!expected.is_empty());
    }
    assert!(published > 0 && published < commits.len());
}

#[test]
fn matches_git_with_a_commit_graph_that_misses_the_newest_commits() {
    check(true);
}

#[test]
fn matches_git_without_a_commit_graph() {
    check(false);
}

/// The default list, as `PROTECTED` in surgery.rs spells it.
fn protected_by_default(short: &str) -> bool {
    let branch = short.split_once('/').map_or(short, |(_, rest)| rest);
    matches!(branch, "main" | "master" | "develop")
        || branch
            .strip_prefix("release/")
            .is_some_and(|rest| !rest.contains('/'))
}

#[test]
fn the_default_patterns_pick_from_the_refs_git_lists() {
    let setup = setup(true);
    let f = &setup.f;
    f.git(&["config", "--unset", "cogit.protectedBranches"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    for oid in every_commit(f) {
        let expected: Vec<String> = containing(f, &oid)
            .into_iter()
            .filter(|name| protected_by_default(name))
            .collect();
        assert_eq!(repo.protecting_refs(&oid).unwrap(), expected, "{oid}");
    }
}

#[test]
fn a_revision_by_name_and_an_unknown_one_behave_like_git() {
    let setup = setup(true);
    let f = &setup.f;
    let repo = RepoHandle::open(f.path()).unwrap();

    assert_eq!(
        repo.protecting_refs("side~1").unwrap(),
        containing(f, &f.oid("side~1").unwrap())
    );
    assert!(repo.protecting_refs("no-such-rev").is_err());
    assert!(repo.is_published("no-such-rev").is_err());
}
