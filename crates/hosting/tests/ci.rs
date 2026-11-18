#![allow(clippy::unwrap_used, clippy::expect_used)]

use hosting::{
    ApiError, CiCache, CiCheck, CiState, CiStatus, HostingAccount, HostingProvider, aggregate,
    auth_for, bitbucket_statuses, commit_page, fetch_checks, github_check_runs, github_statuses,
    gitlab_statuses, ttl,
};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn sample(name: &str) -> Value {
    let path = format!("{}/tests/samples/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn brief(checks: &[CiCheck]) -> Vec<(&str, CiState, Option<u32>)> {
    checks
        .iter()
        .map(|check| (check.name.as_str(), check.state, check.duration_secs))
        .collect()
}

fn check(state: CiState) -> CiCheck {
    CiCheck {
        name: "x".into(),
        state,
        description: None,
        url: None,
        duration_secs: None,
    }
}

#[test]
fn github_statuses_take_error_as_failure() {
    let checks = github_statuses(&sample("github-status.json"));
    assert_eq!(
        brief(&checks),
        [
            ("continuous-integration/jenkins", CiState::Success, None),
            ("security/brakeman", CiState::Failure, None),
            ("deploy/preview", CiState::Pending, None),
        ]
    );
    assert_eq!(
        checks[0].url.as_deref(),
        Some("https://ci.example.com/1000/output")
    );
    assert_eq!(checks[2].url, None);
    assert_eq!(checks[2].description, None);
}

#[test]
fn github_check_runs_read_status_then_conclusion() {
    let checks = github_check_runs(&sample("github-check-runs.json"));
    assert_eq!(
        brief(&checks),
        [
            ("build (ubuntu-latest)", CiState::Success, Some(71)),
            ("test", CiState::Pending, None),
            ("deploy", CiState::Neutral, Some(0)),
            ("e2e", CiState::Failure, Some(3600)),
        ]
    );
    assert_eq!(
        checks[0].url.as_deref(),
        Some("https://github.com/github/hello-world/runs/4")
    );
    assert_eq!(checks[3].url.as_deref(), Some("https://ci.example.com/7"));
    assert_eq!(
        checks[0].description.as_deref(),
        Some("Mighty Readme report")
    );
}

#[test]
fn gitlab_jobs_allowed_to_fail_do_not_fail_the_commit() {
    let checks = gitlab_statuses(&sample("gitlab-statuses.json"));
    assert_eq!(
        brief(&checks),
        [
            ("bundler:audit", CiState::Pending, None),
            ("test", CiState::Failure, Some(210)),
            ("lint", CiState::Neutral, Some(10)),
            ("build", CiState::Success, Some(60)),
            ("deploy", CiState::Neutral, None),
        ]
    );
    assert_eq!(aggregate(&checks), CiState::Failure);
}

#[test]
fn bitbucket_stopped_is_a_failure() {
    let checks = bitbucket_statuses(&sample("bitbucket-statuses.json"));
    assert_eq!(
        brief(&checks),
        [
            ("Build #34", CiState::Success, Some(200)),
            ("pipelines", CiState::Pending, None),
            ("Lint", CiState::Failure, Some(30)),
        ]
    );
}

#[test]
fn an_answer_of_another_shape_has_no_checks() {
    let odd = serde_json::json!({"message": "Not Found"});
    assert!(github_statuses(&odd).is_empty());
    assert!(github_check_runs(&odd).is_empty());
    assert!(gitlab_statuses(&odd).is_empty());
    assert!(bitbucket_statuses(&serde_json::json!([])).is_empty());
}

#[test]
fn a_failure_wins_then_pending_then_success() {
    use CiState::{Failure, Neutral, None, Pending, Success};
    assert_eq!(aggregate(&[]), None);
    assert_eq!(aggregate(&[check(Neutral)]), Neutral);
    assert_eq!(aggregate(&[check(Neutral), check(Success)]), Success);
    assert_eq!(aggregate(&[check(Success), check(Pending)]), Pending);
    assert_eq!(
        aggregate(&[check(Pending), check(Failure), check(Success)]),
        Failure
    );
}

fn status(sha: &str, state: CiState) -> CiStatus {
    CiStatus {
        sha: sha.into(),
        state,
        checks: Vec::new(),
        url: None,
        note: None,
    }
}

#[test]
fn pending_is_polled_with_backoff_and_finished_is_kept_long() {
    assert_eq!(ttl(CiState::Pending, 0), Duration::from_secs(15));
    assert_eq!(ttl(CiState::Pending, 1), Duration::from_secs(30));
    assert_eq!(ttl(CiState::Pending, 10), Duration::from_secs(300));
    assert_eq!(ttl(CiState::None, 0), Duration::from_secs(120));
    assert_eq!(ttl(CiState::None, 9), Duration::from_secs(3600));
    assert_eq!(ttl(CiState::Success, 0), Duration::from_secs(3600));
    assert_eq!(ttl(CiState::Failure, 5), Duration::from_secs(3600));
}

#[test]
fn the_cache_answers_until_the_entry_is_due() {
    let start = Instant::now();
    let mut cache = CiCache::default();
    assert!(cache.lookup("a|o/r", "abc", start).due);
    assert!(cache.claim("a|o/r", "abc", start));
    assert!(!cache.claim("a|o/r", "abc", start), "in flight already");
    cache.store("a|o/r", status("abc", CiState::Pending), start);
    let held = cache.lookup("a|o/r", "abc", start + Duration::from_secs(10));
    assert_eq!(held.status.unwrap().state, CiState::Pending);
    assert!(!held.due);
    assert!(
        cache
            .lookup("a|o/r", "abc", start + Duration::from_secs(15))
            .due
    );
    assert_eq!(
        cache.due_in("a|o/r", "abc", start + Duration::from_secs(5)),
        Some(Duration::from_secs(10))
    );

    let later = start + Duration::from_secs(15);
    assert!(cache.claim("a|o/r", "abc", later));
    cache.store("a|o/r", status("abc", CiState::Pending), later);
    assert!(
        !cache
            .lookup("a|o/r", "abc", later + Duration::from_secs(29))
            .due
    );
    assert!(
        cache
            .lookup("a|o/r", "abc", later + Duration::from_secs(30))
            .due
    );

    let done = later + Duration::from_secs(30);
    cache.store("a|o/r", status("abc", CiState::Success), done);
    assert!(
        !cache
            .lookup("a|o/r", "abc", done + Duration::from_secs(3599))
            .due
    );
    assert_eq!(cache.due_in("a|o/r", "abc", done), None);
    assert!(cache.lookup("b|o/r", "abc", done).status.is_none());
}

#[test]
fn a_failed_request_backs_off_and_keeps_what_was_known() {
    let start = Instant::now();
    let mut cache = CiCache::default();
    cache.store("s", status("abc", CiState::Pending), start);
    let next = start + Duration::from_secs(20);
    assert!(cache.claim("s", "abc", next));
    cache.failed("s", "abc", next);
    let held = cache.lookup("s", "abc", next + Duration::from_secs(299));
    assert_eq!(held.status.unwrap().state, CiState::Pending);
    assert!(!held.due);
    assert!(
        cache
            .lookup("s", "abc", next + Duration::from_secs(300))
            .due
    );
    let again = next + Duration::from_secs(300);
    assert!(cache.claim("s", "abc", again));
    cache.failed("s", "abc", again);
    assert!(
        !cache
            .lookup("s", "abc", again + Duration::from_secs(599))
            .due
    );
}

#[test]
fn a_block_holds_until_its_time() {
    let start = Instant::now();
    let mut cache = CiCache::default();
    cache.block(
        "acct",
        start + Duration::from_secs(60),
        "rate limited".into(),
    );
    assert_eq!(cache.blocked("acct", start), Some("rate limited"));
    assert_eq!(cache.blocked("other", start), None);
    assert_eq!(cache.blocked("acct", start + Duration::from_secs(60)), None);
}

#[test]
fn the_least_recently_used_entries_go_past_the_cap() {
    let start = Instant::now();
    let mut cache = CiCache::with_cap(2);
    cache.store("s", status("a", CiState::Success), start);
    cache.store(
        "s",
        status("b", CiState::Success),
        start + Duration::from_secs(1),
    );
    let _ = cache.lookup("s", "a", start + Duration::from_secs(2));
    cache.store(
        "s",
        status("c", CiState::Success),
        start + Duration::from_secs(3),
    );
    assert_eq!(cache.len(), 2);
    assert!(cache.lookup("s", "a", start).status.is_some());
    assert!(cache.lookup("s", "b", start).status.is_none());
}

struct Route {
    path: &'static str,
    status: u16,
    headers: &'static str,
    body: String,
}

struct Seen {
    path: String,
    headers: Vec<(String, String)>,
}

fn serve(routes: Vec<Route>) -> (String, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (send, seen) = mpsc::channel();
    std::thread::spawn(move || {
        for _ in 0..routes.len() {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    headers.push((name.trim().to_lowercase(), value.trim().to_owned()));
                }
            }
            let route = routes
                .iter()
                .find(|route| path == route.path)
                .expect("an expected path");
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {} X\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                route.status,
                route.headers,
                route.body.len(),
                route.body
            )
            .unwrap();
            send.send(Seen { path, headers }).unwrap();
        }
    });
    (base, seen)
}

fn github_at(base: &str) -> hosting::HostingMatch {
    let account = HostingAccount {
        id: "a".into(),
        provider: HostingProvider::Github,
        server_url: Some(base.to_owned()),
        login: "ann".into(),
        username: None,
        use_for_git: true,
    };
    hosting::account_for(&[account], &format!("{base}/octo/hello.git")).unwrap()
}

const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn raw(name: &str) -> String {
    sample(name).to_string()
}

#[test]
fn github_end_to_end_merges_statuses_and_check_runs() {
    let (base, seen) = serve(vec![
        Route {
            path: "/api/v3/repos/octo/hello/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/status?per_page=100",
            status: 200,
            headers: "X-RateLimit-Remaining: 4000\r\n",
            body: raw("github-status.json"),
        },
        Route {
            path: "/api/v3/repos/octo/hello/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/check-runs?per_page=100",
            status: 200,
            headers: "X-RateLimit-Remaining: 3999\r\nX-RateLimit-Reset: 1900000000\r\n",
            body: raw("github-check-runs.json"),
        },
    ]);
    let found = github_at(&base);
    let auth = auth_for(HostingProvider::Github, None, "ghp_x").unwrap();
    let report = fetch_checks(&found, &auth, None, SHA).unwrap();
    assert_eq!(report.checks.len(), 7);
    assert_eq!(report.checks[0].state, CiState::Failure);
    assert_eq!(aggregate(&report.checks), CiState::Failure);
    assert_eq!(report.limit.remaining, Some(3999));
    assert_eq!(report.limit.reset_at, Some(1_900_000_000));
    assert_eq!(report.note, None);
    let first = seen.recv().unwrap();
    let header = |name: &str| {
        first
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(header("authorization").as_deref(), Some("Bearer ghp_x"));
    assert_eq!(
        header("accept").as_deref(),
        Some("application/vnd.github+json")
    );
    assert_eq!(
        header("x-github-api-version").as_deref(),
        Some("2022-11-28")
    );
    assert!(first.path.ends_with("/status?per_page=100"));
    assert!(
        seen.recv()
            .unwrap()
            .path
            .ends_with("/check-runs?per_page=100")
    );
    assert_eq!(
        commit_page(&found, SHA),
        format!("{base}/octo/hello/commit/{SHA}")
    );
}

#[test]
fn github_without_the_checks_permission_still_shows_statuses() {
    let (base, _seen) = serve(vec![
        Route {
            path: "/api/v3/repos/octo/hello/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/status?per_page=100",
            status: 200,
            headers: "",
            body: raw("github-status.json"),
        },
        Route {
            path: "/api/v3/repos/octo/hello/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/check-runs?per_page=100",
            status: 403,
            headers: "X-RateLimit-Remaining: 4000\r\n",
            body: r#"{"message":"Resource not accessible by personal access token"}"#.into(),
        },
    ]);
    let auth = auth_for(HostingProvider::Github, None, "t").unwrap();
    let report = fetch_checks(&github_at(&base), &auth, None, SHA).unwrap();
    assert_eq!(report.checks.len(), 3);
    assert!(report.note.unwrap().contains("Checks permission"));
}

#[test]
fn a_spent_rate_limit_is_its_own_error() {
    let (base, _seen) = serve(vec![Route {
        path: "/api/v3/repos/octo/hello/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/status?per_page=100",
        status: 403,
        headers: "X-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: 1\r\n",
        body: r#"{"message":"API rate limit exceeded"}"#.into(),
    }]);
    let auth = auth_for(HostingProvider::Github, None, "t").unwrap();
    let error = fetch_checks(&github_at(&base), &auth, None, SHA).unwrap_err();
    assert!(
        matches!(error, ApiError::RateLimited(_, Some(0))),
        "{error:?}"
    );
}

#[test]
fn too_many_requests_waits_for_retry_after() {
    let (base, _seen) = serve(vec![Route {
        path: "/api/v4/projects/grp%2Fsub%2Frepo/repository/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/statuses?per_page=100",
        status: 429,
        headers: "Retry-After: 42\r\n",
        body: "{}".into(),
    }]);
    let account = HostingAccount {
        id: "g".into(),
        provider: HostingProvider::Gitlab,
        server_url: Some(base.clone()),
        login: "bob".into(),
        username: None,
        use_for_git: true,
    };
    let found = hosting::account_for(&[account], &format!("{base}/grp/sub/repo.git")).unwrap();
    let auth = auth_for(HostingProvider::Gitlab, None, "t").unwrap();
    let error = fetch_checks(&found, &auth, None, SHA).unwrap_err();
    assert!(
        matches!(error, ApiError::RateLimited(_, Some(42))),
        "{error:?}"
    );
    assert!(error.to_string().contains("42 s"));
}

#[test]
fn gitlab_end_to_end_reads_jobs_of_an_encoded_project() {
    let (base, seen) = serve(vec![Route {
        path: "/api/v4/projects/grp%2Frepo/repository/commits/6dcb09b5b57875f334f61aebed695e2e4193db5e/statuses?per_page=100",
        status: 200,
        headers: "RateLimit-Remaining: 1999\r\n",
        body: raw("gitlab-statuses.json"),
    }]);
    let account = HostingAccount {
        id: "g".into(),
        provider: HostingProvider::Gitlab,
        server_url: Some(base.clone()),
        login: "bob".into(),
        username: None,
        use_for_git: true,
    };
    let found = hosting::account_for(&[account], &format!("{base}/grp/repo.git")).unwrap();
    let auth = auth_for(HostingProvider::Gitlab, None, "t").unwrap();
    let report = fetch_checks(&found, &auth, None, SHA).unwrap();
    assert_eq!(aggregate(&report.checks), CiState::Failure);
    assert_eq!(report.limit.remaining, Some(1999));
    assert_eq!(
        seen.recv()
            .unwrap()
            .headers
            .iter()
            .find(|(k, _)| k == "accept")
            .unwrap()
            .1,
        "application/json"
    );
    assert_eq!(
        commit_page(&found, SHA),
        format!("{base}/grp/repo/-/commit/{SHA}/pipelines")
    );
}

#[test]
fn a_ref_name_never_reaches_the_server() {
    let found = github_at("http://127.0.0.1:9");
    let auth = auth_for(HostingProvider::Github, None, "t").unwrap();
    let error = fetch_checks(&found, &auth, None, "../../user").unwrap_err();
    assert!(matches!(error, ApiError::Reply(_)), "{error:?}");
}
