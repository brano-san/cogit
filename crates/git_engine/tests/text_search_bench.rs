// Timing harness, not a test: `cargo test -p git_engine --release --test text_search_bench
// -- --ignored --nocapture` with COGIT_TS_BENCH_REPOS="name=path;name=path" (R-625).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::cast_precision_loss
)]

use std::time::Instant;

use git_engine::{CommitQuery, RepoHandle, TextFields};

const NONE: TextFields = TextFields {
    author: false,
    committer: false,
    message: false,
    refs: false,
    id: false,
    name: false,
    content: false,
    notes: false,
};

fn needle(repo: &str, field: &str) -> &'static str {
    if repo.starts_with("cogit") {
        return match field {
            "author" | "committer" => "brano",
            "message" => "fix",
            "refs" => "master",
            "id" => "13a7",
            "name" => "text_search",
            "content" => "TextFields",
            "notes" => "needle_n1",
            _ => "fix",
        };
    }
    match field {
        "author" | "committer" => "bench",
        "name" => "file0192",
        "notes" => "needle_n12",
        _ => "1234",
    }
}

fn run(repo: &RepoHandle, text: &str, fields: TextFields) -> (f64, usize) {
    let query = CommitQuery {
        text: Some(text.to_owned()),
        text_in: fields,
        ..CommitQuery::default()
    };
    let start = Instant::now();
    let mut rows = 0;
    repo.search_commits(&query, 200, |chunk| {
        rows += chunk.len();
        true
    })
    .unwrap();
    (start.elapsed().as_secs_f64() * 1000.0, rows)
}

#[test]
#[ignore = "timing harness"]
fn time_each_field() {
    let repos = std::env::var("COGIT_TS_BENCH_REPOS").expect("COGIT_TS_BENCH_REPOS");
    let runs: usize = std::env::var("COGIT_TS_BENCH_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let only = std::env::var("COGIT_TS_BENCH_ONLY").unwrap_or_default();
    let on = |f: &str| TextFields {
        author: f == "author",
        committer: f == "committer",
        message: f == "message",
        refs: f == "refs",
        id: f == "id",
        name: f == "name",
        content: f == "content",
        notes: f == "notes",
    };
    let mut configs: Vec<(String, TextFields)> = vec![("none".into(), NONE)];
    for f in [
        "author",
        "committer",
        "message",
        "refs",
        "id",
        "name",
        "content",
        "notes",
    ] {
        configs.push((f.into(), on(f)));
    }
    configs.push((
        "default5".into(),
        TextFields {
            notes: false,
            ..TextFields::default()
        },
    ));
    configs.push((
        "default+notes".into(),
        TextFields {
            notes: true,
            ..TextFields::default()
        },
    ));
    println!("repo\tfield\tquery\trows\tcold\tmedian\tp95");
    for spec in repos.split(';').filter(|s| !s.is_empty()) {
        let (name, path) = spec.split_once('=').unwrap();
        for (label, fields) in &configs {
            if !only.is_empty() && !only.split(',').any(|o| o == label) {
                continue;
            }
            for matching in [true, false] {
                let text = if matching {
                    needle(name, label).to_owned()
                } else {
                    "zzqqxx_nomatch".to_owned()
                };
                let handle = RepoHandle::open(std::path::Path::new(path)).unwrap();
                let (cold, rows) = run(&handle, &text, *fields);
                let mut times: Vec<f64> =
                    (0..runs).map(|_| run(&handle, &text, *fields).0).collect();
                times.sort_by(f64::total_cmp);
                let p95 = times[((times.len() * 95).div_ceil(100)).saturating_sub(1)];
                println!(
                    "{name}\t{label}\t{}\t{rows}\t{cold:.1}\t{:.1}\t{p95:.1}",
                    if matching { "match" } else { "miss" },
                    times[times.len() / 2]
                );
            }
        }
    }
}
