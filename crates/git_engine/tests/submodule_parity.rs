// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The submodule tree against `git submodule status --recursive` (R-355): the same
//! submodules, at any depth, with the same mark (` ` in step, `-` not initialised, `+`
//! checked out elsewhere) and the same commit.

use git_engine::{RepoHandle, Submodule, SubmoduleState};
use test_fixtures::Fixture;

fn mark(module: &Submodule) -> char {
    match module.state {
        SubmoduleState::InSync => ' ',
        SubmoduleState::NotInitialised => '-',
        _ => '+',
    }
}

/// What git prints for a `+` is the commit checked out; otherwise the one recorded.
fn line(prefix: &str, module: &Submodule) -> String {
    let mark = mark(module);
    let oid = if mark == '+' {
        module.checked_out.clone().unwrap_or_default()
    } else {
        module.recorded.clone()
    };
    format!("{mark}{oid} {prefix}{}", module.path)
}

/// Every level, as the tree reads it: one listing per repository, nested ones opened
/// where the parent says they are.
fn ours(root: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
    let handle = RepoHandle::open_exact(root).unwrap();
    for module in handle.submodules().unwrap() {
        out.push(line(prefix, &module));
        if module.state != SubmoduleState::NotInitialised && module.nested {
            let inner = format!("{prefix}{}/", module.path);
            ours(&root.join(&module.path), &inner, out);
        }
    }
}

fn git(f: &Fixture) -> Vec<String> {
    f.git(&["submodule", "status", "--recursive"])
        .unwrap()
        .lines()
        .map(|line| {
            // `(describe)` after the path is git's own decoration.
            match line.rfind(" (") {
                Some(cut) if line.ends_with(')') => line[..cut].to_owned(),
                _ => line.to_owned(),
            }
        })
        .collect()
}

fn listed(f: &Fixture) -> Vec<String> {
    let mut out = Vec::new();
    ours(f.path(), "", &mut out);
    out
}

fn commit_inside(f: &Fixture, inner: &std::path::Path) {
    let identity = ["-c", "user.name=T", "-c", "user.email=t@cogit.invalid"];
    let mut args = identity.to_vec();
    args.extend(["commit", "--quiet", "--allow-empty", "-m", "inside"]);
    f.git_in(inner, &args).unwrap();
}

#[test]
fn in_step() {
    let f = test_fixtures::with_submodule().unwrap();
    assert_eq!(listed(&f), git(&f));
}

#[test]
fn not_initialised() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["submodule", "deinit", "-f", "--", "vendor/lib"])
        .unwrap();
    assert_eq!(listed(&f), git(&f));
    assert!(listed(&f)[0].starts_with('-'));
}

#[test]
fn ahead_of_the_recorded_commit() {
    let f = test_fixtures::with_submodule().unwrap();
    commit_inside(&f, &f.path().join("vendor/lib"));
    assert_eq!(listed(&f), git(&f));
    assert!(listed(&f)[0].starts_with('+'));
}

#[test]
fn behind_the_recorded_commit() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git_in(
        &f.path().join("vendor/lib"),
        &["checkout", "--quiet", "--detach", "HEAD~1"],
    )
    .unwrap();
    assert_eq!(listed(&f), git(&f));
}

#[test]
fn two_levels_deep() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    assert_eq!(listed(&f).len(), 2);
    assert_eq!(listed(&f), git(&f));
}

#[test]
fn two_levels_deep_with_the_inner_one_moved() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    commit_inside(&f, &f.path().join("vendor/middle/deep/inner"));
    assert_eq!(listed(&f), git(&f));
}

/// The light outline of a repository not on screen (R-352) opens no submodule, so it can
/// only agree on which are initialised and on the commit recorded for those that are not.
#[test]
fn the_outline_agrees_where_it_can() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    f.git_in(
        &f.path().join("vendor/middle"),
        &["submodule", "deinit", "-f", "--", "deep/inner"],
    )
    .unwrap();
    let outline = RepoHandle::open(f.path())
        .unwrap()
        .submodule_outline()
        .unwrap();
    let inner = RepoHandle::open_exact(&f.path().join("vendor/middle"))
        .unwrap()
        .submodule_outline()
        .unwrap();
    let lines = git(&f);
    assert_eq!(outline[0].state, SubmoduleState::Unread);
    assert!(lines[0].starts_with(' ') && lines[0].ends_with(" vendor/middle"));
    assert_eq!(inner[0].state, SubmoduleState::NotInitialised);
    assert_eq!(
        lines[1],
        format!("-{} vendor/middle/deep/inner", inner[0].recorded)
    );
}

// Where they part, pinned so a change to either side shows. Git reads the commit from the
// index, Cogit from HEAD: a pointer staged but not committed is "in step" to git and
// "ahead" to Cogit, whose row asks for exactly that commit.
#[test]
fn a_staged_pointer_is_where_cogit_and_git_part() {
    let f = test_fixtures::with_submodule().unwrap();
    commit_inside(&f, &f.path().join("vendor/lib"));
    f.git(&["add", "--", "vendor/lib"]).unwrap();
    assert!(git(&f)[0].starts_with(' '));
    assert!(listed(&f)[0].starts_with('+'));
}

// Git asks `submodule.<name>.active` (or a URL in the config); Cogit asks whether a
// repository is checked out there. Switched off but still on disk, git says `-`.
#[test]
fn an_inactive_checkout_is_where_cogit_and_git_part() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["config", "submodule.vendor/lib.active", "false"])
        .unwrap();
    assert!(git(&f)[0].starts_with('-'));
    assert!(listed(&f)[0].starts_with(' '));
}

/// On the benchmark's `submodules` set, run by hand: `COGIT_BENCH_SUBMODULES=… cargo
/// nextest run -p git_engine --test submodule_parity --run-ignored only --no-capture`.
#[test]
#[ignore = "needs the benchmark's submodules set in COGIT_BENCH_SUBMODULES"]
#[allow(clippy::print_stderr)]
fn the_tree_against_git_submodule_status_takes() {
    let Some(path) = std::env::var_os("COGIT_BENCH_SUBMODULES") else {
        return;
    };
    let root = std::path::PathBuf::from(path);
    let median = |work: &dyn Fn()| {
        let mut times: Vec<u128> = (0..11)
            .map(|_| {
                let started = std::time::Instant::now();
                work();
                started.elapsed().as_micros()
            })
            .collect();
        times.sort_unstable();
        (times[5], times[9])
    };
    let full = median(&|| {
        let mut out = Vec::new();
        ours(&root, "", &mut out);
    });
    let outline = median(&|| {
        RepoHandle::open(&root)
            .unwrap()
            .submodule_outline()
            .unwrap();
    });
    let git = median(&|| {
        let status = std::process::Command::new("git")
            .args(["submodule", "status", "--recursive"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(status.status.success());
    });
    eprintln!("median/p90 µs: tree {full:?}, outline {outline:?}, git submodule status {git:?}");
    let printed = std::process::Command::new("git")
        .args(["submodule", "status", "--recursive"])
        .current_dir(&root)
        .output()
        .unwrap();
    let theirs: Vec<String> = String::from_utf8_lossy(&printed.stdout)
        .lines()
        .map(|line| line.rfind(" (").map_or(line, |cut| &line[..cut]).to_owned())
        .collect();
    let mut mine = Vec::new();
    ours(&root, "", &mut mine);
    assert_eq!(mine, theirs);
}
