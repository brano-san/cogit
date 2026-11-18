use crate::account::HostingProvider;
use crate::api::{ApiError, Auth, Limit, get_reply};
use crate::remote::HostingMatch;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CiState {
    Success,
    Failure,
    Pending,
    Neutral,
    None,
}

impl CiState {
    #[must_use]
    pub fn finished(self) -> bool {
        matches!(self, Self::Success | Self::Failure | Self::Neutral)
    }

    fn rank(self) -> u8 {
        match self {
            Self::Failure => 0,
            Self::Pending => 1,
            Self::Success => 2,
            Self::Neutral => 3,
            Self::None => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CiCheck {
    pub name: String,
    pub state: CiState,
    pub description: Option<String>,
    pub url: Option<String>,
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CiStatus {
    pub sha: String,
    pub state: CiState,
    pub checks: Vec<CiCheck>,
    pub url: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct CiReport {
    pub checks: Vec<CiCheck>,
    pub limit: Limit,
    pub note: Option<String>,
}

#[must_use]
pub fn aggregate(checks: &[CiCheck]) -> CiState {
    checks
        .iter()
        .map(|check| check.state)
        .min_by_key(|state| state.rank())
        .unwrap_or(CiState::None)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn duration(started: Option<String>, finished: Option<String>) -> Option<u32> {
    let started = chrono::DateTime::parse_from_rfc3339(&started?).ok()?;
    let finished = chrono::DateTime::parse_from_rfc3339(&finished?).ok()?;
    u32::try_from((finished - started).num_seconds()).ok()
}

fn sorted(mut checks: Vec<CiCheck>) -> Vec<CiCheck> {
    checks.sort_by(|a, b| {
        a.state
            .rank()
            .cmp(&b.state.rank())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    checks
}

fn items<'a>(value: &'a Value, key: Option<&str>) -> &'a [Value] {
    let list = match key {
        Some(key) => value.get(key),
        None => Some(value),
    };
    list.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

#[must_use]
pub fn github_statuses(reply: &Value) -> Vec<CiCheck> {
    items(reply, Some("statuses"))
        .iter()
        .map(|status| CiCheck {
            name: text(status, "context").unwrap_or_else(|| "status".to_owned()),
            state: match status.get("state").and_then(Value::as_str) {
                Some("success") => CiState::Success,
                Some("failure" | "error") => CiState::Failure,
                _ => CiState::Pending,
            },
            description: text(status, "description"),
            url: text(status, "target_url"),
            duration_secs: None,
        })
        .collect()
}

#[must_use]
pub fn github_check_runs(reply: &Value) -> Vec<CiCheck> {
    items(reply, Some("check_runs"))
        .iter()
        .map(|run| {
            let completed = run.get("status").and_then(Value::as_str) == Some("completed");
            let state = if completed {
                match run.get("conclusion").and_then(Value::as_str) {
                    Some("success") => CiState::Success,
                    Some(
                        "failure" | "cancelled" | "timed_out" | "action_required"
                        | "startup_failure",
                    ) => CiState::Failure,
                    _ => CiState::Neutral,
                }
            } else {
                CiState::Pending
            };
            CiCheck {
                name: text(run, "name").unwrap_or_else(|| "check".to_owned()),
                state,
                description: run.get("output").and_then(|output| text(output, "title")),
                url: text(run, "html_url").or_else(|| text(run, "details_url")),
                duration_secs: completed
                    .then(|| duration(text(run, "started_at"), text(run, "completed_at")))
                    .flatten(),
            }
        })
        .collect()
}

#[must_use]
pub fn gitlab_statuses(reply: &Value) -> Vec<CiCheck> {
    items(reply, None)
        .iter()
        .map(|job| {
            let allowed = job.get("allow_failure").and_then(Value::as_bool) == Some(true);
            let state = match job.get("status").and_then(Value::as_str) {
                Some("success") => CiState::Success,
                Some("failed") if allowed => CiState::Neutral,
                Some("failed" | "canceled" | "canceling") => CiState::Failure,
                Some("skipped" | "manual") => CiState::Neutral,
                _ => CiState::Pending,
            };
            CiCheck {
                name: text(job, "name").unwrap_or_else(|| "job".to_owned()),
                state,
                description: text(job, "description"),
                url: text(job, "target_url"),
                duration_secs: state
                    .finished()
                    .then(|| duration(text(job, "started_at"), text(job, "finished_at")))
                    .flatten(),
            }
        })
        .collect()
}

#[must_use]
pub fn bitbucket_statuses(reply: &Value) -> Vec<CiCheck> {
    items(reply, Some("values"))
        .iter()
        .map(|status| {
            let state = match status.get("state").and_then(Value::as_str) {
                Some("SUCCESSFUL") => CiState::Success,
                Some("FAILED" | "STOPPED") => CiState::Failure,
                _ => CiState::Pending,
            };
            CiCheck {
                name: text(status, "name")
                    .or_else(|| text(status, "key"))
                    .unwrap_or_else(|| "build".to_owned()),
                state,
                description: text(status, "description"),
                url: text(status, "url"),
                duration_secs: state
                    .finished()
                    .then(|| duration(text(status, "created_on"), text(status, "updated_on")))
                    .flatten(),
            }
        })
        .collect()
}

fn encode(component: &str) -> String {
    component
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn valid_sha(sha: &str) -> bool {
    (4..=64).contains(&sha.len()) && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[must_use]
pub fn commit_page(found: &HostingMatch, sha: &str) -> String {
    match found.account.provider {
        HostingProvider::Github => format!("{}/commit/{sha}", found.web_url),
        HostingProvider::Gitlab => format!("{}/-/commit/{sha}/pipelines", found.web_url),
        HostingProvider::Bitbucket => format!("{}/commits/{sha}", found.web_url),
    }
}

const GITHUB_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];

fn lower(a: Limit, b: Limit) -> Limit {
    match (a.remaining, b.remaining) {
        (Some(x), Some(y)) if x <= y => a,
        (Some(_), None) => a,
        _ => b,
    }
}

pub fn fetch_checks(
    found: &HostingMatch,
    auth: &Auth,
    proxy: Option<&str>,
    sha: &str,
) -> Result<CiReport, ApiError> {
    if !valid_sha(sha) {
        return Err(ApiError::Reply(sha.to_owned()));
    }
    let api = found.api_url.trim_end_matches('/');
    let repository = &found.repository;
    match found.account.provider {
        HostingProvider::Github => {
            let base = format!("{api}/repos/{repository}/commits/{sha}");
            let statuses = get_reply(
                &format!("{base}/status?per_page=100"),
                auth,
                proxy,
                GITHUB_HEADERS,
            )?;
            let mut checks = github_statuses(&statuses.body);
            let mut limit = statuses.limit;
            let mut note = None;
            match get_reply(
                &format!("{base}/check-runs?per_page=100"),
                auth,
                proxy,
                GITHUB_HEADERS,
            ) {
                Ok(runs) => {
                    checks.extend(github_check_runs(&runs.body));
                    limit = lower(limit, runs.limit);
                }
                Err(ApiError::Refused(_, 403)) => {
                    note = Some(
                        "The token may not read check runs (fine-grained tokens need the Checks permission): only commit statuses are shown."
                            .to_owned(),
                    );
                }
                Err(err) => return Err(err),
            }
            Ok(CiReport {
                checks: sorted(checks),
                limit,
                note,
            })
        }
        HostingProvider::Gitlab => {
            let url = format!(
                "{api}/projects/{}/repository/commits/{sha}/statuses?per_page=100",
                encode(repository)
            );
            let reply = get_reply(&url, auth, proxy, &[])?;
            Ok(CiReport {
                checks: sorted(gitlab_statuses(&reply.body)),
                limit: reply.limit,
                note: None,
            })
        }
        HostingProvider::Bitbucket => {
            let url = format!("{api}/repositories/{repository}/commit/{sha}/statuses?pagelen=100");
            let reply = get_reply(&url, auth, proxy, &[])?;
            Ok(CiReport {
                checks: sorted(bitbucket_statuses(&reply.body)),
                limit: reply.limit,
                note: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gitlab_project_path_is_one_encoded_segment() {
        assert_eq!(
            encode("group/sub group/repo.x"),
            "group%2Fsub%20group%2Frepo.x"
        );
    }

    #[test]
    fn only_a_hex_sha_reaches_a_url() {
        assert!(valid_sha("0123abcdef"));
        assert!(!valid_sha("../x"));
        assert!(!valid_sha("main"));
    }

    #[test]
    fn durations_come_from_both_ends() {
        assert_eq!(
            duration(
                Some("2026-11-18T10:00:00Z".into()),
                Some("2026-11-18T10:02:05.5+00:00".into())
            ),
            Some(125)
        );
        assert_eq!(duration(Some("2026-11-18T10:00:00Z".into()), None), None);
        assert_eq!(
            duration(
                Some("2026-11-18T10:00:00Z".into()),
                Some("2026-11-18T09:00:00Z".into())
            ),
            None
        );
    }
}
