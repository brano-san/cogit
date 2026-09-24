// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The graph against the console `git`, with a commit-graph file, a stale one and none.
//!
//! The graph's order is `--date-order` with HEAD first (R-162, R-164), not `--topo-order`,
//! so against `--topo-order` the commits and their parents are compared as a set and the
//! order only as a topological one; against `--date-order` it is compared line for line
//! where no two commits share a second and HEAD is the newest tip.

use app_state::{AppState, RepoId};
use git_engine::CommitQuery;
use std::collections::{HashMap, HashSet};

struct Row {
    oid: String,
    parents: Vec<String>,
    text: String,
}

fn graph(state: &AppState, repo: RepoId, query: &CommitQuery) -> Vec<Row> {
    let generation = state.begin_graph();
    state
        .build_graph(repo, query, generation, 7, |_| true)
        .unwrap();
    let mut rows = Vec::new();
    let mut start = 0;
    loop {
        // Small windows, so a lazily read row is read by more than one of them.
        let window = state.graph_window(repo, generation, start, 5).unwrap();
        if window.commits.is_empty() {
            break;
        }
        start += u32::try_from(window.commits.len()).unwrap();
        rows.extend(window.commits.into_iter().map(|c| Row {
            text: format!(
                "{} {} <{}> {} {}",
                c.summary, c.author_name, c.author_email, c.timestamp, c.tz_offset_minutes
            ),
            oid: c.oid,
            parents: c.parents,
        }));
    }
    rows
}

fn git_lines(f: &test_fixtures::Fixture, order: &str, extra: &[&str]) -> Vec<String> {
    let mut args = vec![
        "log",
        order,
        "--format=%H %P",
        "--branches",
        "--remotes",
        "HEAD",
    ];
    args.extend_from_slice(extra);
    f.git(&args)
        .unwrap()
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

fn lines(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            std::iter::once(row.oid.as_str())
                .chain(row.parents.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn assert_topological(rows: &[Row]) {
    let at: HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.oid.as_str(), i))
        .collect();
    for (i, row) in rows.iter().enumerate() {
        for parent in &row.parents {
            if let Some(&p) = at.get(parent.as_str()) {
                assert!(p > i, "{parent} above its child {}", row.oid);
            }
        }
    }
}

/// Subject, author, committer time and offset, as the graph shows them.
fn git_text(f: &test_fixtures::Fixture) -> HashMap<String, String> {
    f.git(&[
        "log",
        "--format=%H%x00%s%x00%an%x00%ae%x00%ct%x00%ci",
        "--branches",
        "--remotes",
        "HEAD",
    ])
    .unwrap()
    .lines()
    .map(|line| {
        let parts: Vec<&str> = line.split('\0').collect();
        let zone = parts[5].rsplit(' ').next().unwrap();
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        let hours: i32 = zone[1..3].parse().unwrap();
        let minutes: i32 = zone[3..5].parse().unwrap();
        let offset = sign * (hours * 60 + minutes);
        (
            parts[0].to_owned(),
            format!(
                "{} {} <{}> {} {offset}",
                parts[1], parts[2], parts[3], parts[4]
            ),
        )
    })
    .collect()
}

fn check(f: &test_fixtures::Fixture, distinct_times: bool) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let rows = graph(&state, repo, &CommitQuery::default());

    let ours: HashSet<String> = lines(&rows).into_iter().collect();
    let theirs: HashSet<String> = git_lines(f, "--topo-order", &[]).into_iter().collect();
    assert_eq!(ours, theirs, "commits and parents against --topo-order");
    assert_eq!(rows.len(), ours.len(), "each commit once");
    assert_topological(&rows);
    if distinct_times {
        assert_eq!(
            lines(&rows),
            git_lines(f, "--date-order", &[]),
            "against --date-order"
        );
    }

    let text = git_text(f);
    for row in &rows {
        assert_eq!(row.text, text[&row.oid], "{}", row.oid);
    }

    let first = graph(
        &state,
        repo,
        &CommitQuery {
            view: git_engine::GraphView {
                first_parent: true,
                ..git_engine::GraphView::default()
            },
            ..CommitQuery::default()
        },
    );
    // `%P` still lists a merge's every parent; the graph draws the first line only (#26).
    let first_line = |line: &str| line.split(' ').take(2).collect::<Vec<_>>().join(" ");
    let ours: HashSet<String> = lines(&first).iter().map(|l| first_line(l)).collect();
    let theirs: HashSet<String> = git_lines(f, "--topo-order", &["--first-parent"])
        .iter()
        .map(|l| first_line(l))
        .collect();
    assert_eq!(ours, theirs, "--first-parent");
    assert_eq!(first.len(), ours.len());
    if distinct_times {
        let ours: Vec<String> = lines(&first).iter().map(|l| first_line(l)).collect();
        let theirs: Vec<String> = git_lines(f, "--date-order", &["--first-parent"])
            .iter()
            .map(|l| first_line(l))
            .collect();
        assert_eq!(ours, theirs, "--first-parent --date-order");
    }
    assert_topological(&first);
}

/// Branches forking and merging, an octopus, a second root, a remote-tracking branch;
/// every commit in a second of its own unless `ties`.
fn history(ties: bool) -> test_fixtures::Fixture {
    let f = test_fixtures::linear(4).unwrap();
    let tie = |i: i64| if ties { 5 } else { i };
    f.git(&["switch", "-c", "feat", "main~2"]).unwrap();
    f.commit_file(tie(5), "f1.txt", "f1\n").unwrap();
    f.commit_file(6, "f2.txt", "f2\n").unwrap();
    f.git(&["switch", "-c", "side", "main"]).unwrap();
    f.commit_file(tie(7), "s1.txt", "s1\n").unwrap();
    f.git(&["switch", "-c", "other", "main~1"]).unwrap();
    f.commit_file(8, "o1.txt", "o1\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(9, "m.txt", "m\n").unwrap();
    f.merge(10, &["side", "other"], "octopus").unwrap();
    f.git(&["switch", "--orphan", "root2"]).unwrap();
    f.git(&["rm", "-rf", "--cached", "--quiet", "--ignore-unmatch", "."])
        .unwrap();
    f.commit_file(11, "r.txt", "r\n").unwrap();
    f.git(&["update-ref", "refs/remotes/origin/feat", "feat"])
        .unwrap();
    f.git(&["switch", "-f", "main"]).unwrap();
    f.commit_file(12, "top.txt", "top\n").unwrap();
    f
}

#[test]
fn without_a_commit_graph_the_graph_is_what_git_log_lists() {
    check(&history(false), true);
    check(&history(true), false);
}

#[test]
fn with_a_commit_graph_the_graph_is_what_git_log_lists() {
    let f = history(false);
    f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    check(&f, true);
}

/// Commits newer than the commit-graph file are read from the objects.
#[test]
fn with_a_stale_commit_graph_the_graph_is_what_git_log_lists() {
    let f = history(false);
    f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    f.git(&["switch", "feat"]).unwrap();
    f.commit_file(13, "late.txt", "late\n").unwrap();
    f.merge(14, &["main"], "merge main into feat late").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.merge(15, &["feat"], "merge feat back").unwrap();
    check(&f, true);
}
