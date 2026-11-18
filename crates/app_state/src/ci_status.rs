use crate::credentials::with_store_in;
use crate::hosting_accounts::accounts_in;
use crate::password_store::StoreState;
use crate::{AppState, RepoId};
use git_engine::GitError;
use hosting::{ApiError, Auth, CiStatus, HostingMatch};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const MAX_TARGETS: usize = 300;
const MAX_FETCHES: usize = 16;
const WORKERS: usize = 4;
const SOON: Duration = Duration::from_millis(500);
const REFUSED_PAUSE: Duration = Duration::from_secs(30 * 60);
const LIMIT_PAUSE: Duration = Duration::from_secs(60);
const LOW_REMAINING: u64 = 10;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CiTarget {
    pub sha: String,
    pub remote: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CiBatch {
    pub statuses: Vec<CiStatus>,
    pub retry_ms: Option<u32>,
}

struct Source {
    key: String,
    account: String,
    found: HostingMatch,
    auth: Auth,
}

type Sources = (Vec<Source>, HashMap<Option<String>, usize>);

struct Job {
    source: usize,
    sha: String,
}

fn source_key(found: &HostingMatch) -> String {
    format!("{}|{}", found.account.id, found.repository.to_lowercase())
}

fn pause_reason(error: &ApiError, until_secs: u64) -> String {
    let minutes = until_secs.div_ceil(60).max(1);
    match error {
        ApiError::RateLimited(site, _) => {
            format!("{site} rate limit reached: CI status is checked again in {minutes} min.")
        }
        ApiError::Refused(site, status) => format!(
            "{site} refused the token (HTTP {status}): CI status is not asked for {minutes} min. Check the account in Preferences ▸ Hosting Providers."
        ),
        other => other.to_string(),
    }
}

fn wait_for(error: &ApiError) -> Option<Duration> {
    match error {
        ApiError::RateLimited(_, wait) => Some(
            wait.map_or(LIMIT_PAUSE, |secs| Duration::from_secs(secs.max(1)))
                .max(Duration::from_secs(5)),
        ),
        ApiError::Refused(..) => Some(REFUSED_PAUSE),
        _ => None,
    }
}

fn missing(error: &ApiError) -> bool {
    matches!(error, ApiError::Status(_, 404 | 409 | 422, _))
}

fn retry_ms(wait: Duration) -> u32 {
    u32::try_from(wait.as_millis()).unwrap_or(u32::MAX)
}

fn with_note(mut status: CiStatus, note: Option<&str>) -> CiStatus {
    if let Some(note) = note {
        status.note = Some(note.to_owned());
    }
    status
}

fn fetch_all(
    sources: &[Source],
    jobs: Vec<Job>,
    proxy: Option<&str>,
) -> Vec<(Job, Option<Result<hosting::CiReport, ApiError>>)> {
    let queue = parking_lot::Mutex::new(jobs);
    let stopped = parking_lot::Mutex::new(HashSet::<usize>::new());
    let results = parking_lot::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                loop {
                    let Some(job) = queue.lock().pop() else {
                        break;
                    };
                    let Some(source) = sources.get(job.source) else {
                        continue;
                    };
                    if stopped.lock().contains(&job.source) {
                        results.lock().push((job, None));
                        continue;
                    }
                    let report =
                        hosting::fetch_checks(&source.found, &source.auth, proxy, &job.sha);
                    let stop = match &report {
                        Ok(report) => report
                            .limit
                            .remaining
                            .is_some_and(|left| left < LOW_REMAINING),
                        Err(error) => wait_for(error).is_some(),
                    };
                    if stop {
                        stopped.lock().insert(job.source);
                    }
                    results.lock().push((job, Some(report)));
                }
            });
        }
    });
    results.into_inner()
}

impl AppState {
    fn ci_sources(
        &self,
        repo: RepoId,
        wanted: &HashSet<Option<String>>,
    ) -> Result<Sources, GitError> {
        let unlocked = with_store_in(&self.passwords, |store| Ok(store.state()))
            .is_ok_and(|state| state == StoreState::Unlocked);
        if !unlocked {
            return Ok((Vec::new(), HashMap::new()));
        }
        let entries = match self.read_entries() {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(error = ?err, context = "reading hosting accounts for CI status");
                return Ok((Vec::new(), HashMap::new()));
            }
        };
        let accounts = accounts_in(&entries);
        if accounts.is_empty() {
            return Ok((Vec::new(), HashMap::new()));
        }
        let names = self.remotes(repo)?;
        let fallback = names
            .iter()
            .find(|name| *name == "origin")
            .or_else(|| names.first())
            .cloned();
        let mut sources: Vec<Source> = Vec::new();
        let mut by_remote = HashMap::new();
        for asked in wanted {
            let Some(name) = asked.clone().or_else(|| fallback.clone()) else {
                continue;
            };
            if !names.contains(&name) {
                continue;
            }
            let Some(url) = self.remote_url(repo, &name)? else {
                continue;
            };
            let Some(found) = hosting::account_for(&accounts, &url) else {
                continue;
            };
            let key = source_key(&found);
            if let Some(at) = sources.iter().position(|source| source.key == key) {
                by_remote.insert(asked.clone(), at);
                continue;
            }
            let Some(token) = entries.iter().find_map(|entry| {
                entry
                    .account
                    .as_ref()
                    .filter(|account| account.id == found.account.id)
                    .map(|_| zeroize::Zeroizing::new(entry.secret.clone()))
            }) else {
                continue;
            };
            let auth = match hosting::auth_for(
                found.account.provider,
                found.account.username.as_deref(),
                &token,
            ) {
                Ok(auth) => auth,
                Err(err) => {
                    tracing::error!(error = ?err, context = "hosting account sign-in for CI status");
                    continue;
                }
            };
            by_remote.insert(asked.clone(), sources.len());
            sources.push(Source {
                key,
                account: found.account.id.clone(),
                found,
                auth,
            });
        }
        Ok((sources, by_remote))
    }

    pub fn ci_statuses(&self, repo: RepoId, targets: &[CiTarget]) -> Result<CiBatch, GitError> {
        let mut seen = HashSet::new();
        let targets: Vec<&CiTarget> = targets
            .iter()
            .filter(|target| seen.insert((target.sha.to_lowercase(), target.remote.clone())))
            .take(MAX_TARGETS)
            .collect();
        if targets.is_empty() {
            return Ok(CiBatch::default());
        }
        let wanted: HashSet<Option<String>> =
            targets.iter().map(|target| target.remote.clone()).collect();
        let (sources, by_remote) = self.ci_sources(repo, &wanted)?;
        if sources.is_empty() {
            return Ok(CiBatch::default());
        }

        let now = Instant::now();
        let mut jobs = Vec::new();
        let mut more = false;
        {
            let mut cache = self.ci.lock();
            for target in &targets {
                let Some(&at) = by_remote.get(&target.remote) else {
                    continue;
                };
                let Some(source) = sources.get(at) else {
                    continue;
                };
                let sha = target.sha.to_lowercase();
                let lookup = cache.lookup(&source.key, &sha, now);
                let blocked = cache.blocked(&source.account, now).is_some();
                if !lookup.due || blocked {
                    continue;
                }
                if jobs.len() >= MAX_FETCHES {
                    more = true;
                    continue;
                }
                if cache.claim(&source.key, &sha, now) {
                    jobs.push(Job { source: at, sha });
                }
            }
        }

        if !jobs.is_empty() {
            let started = Instant::now();
            let count = jobs.len();
            let proxy = crate::hosting_proxy();
            let results = fetch_all(&sources, jobs, proxy.as_deref());
            tracing::info!(count, elapsed = ?started.elapsed(), "CI status fetched");
            let now = Instant::now();
            let mut cache = self.ci.lock();
            for (job, result) in results {
                let Some(source) = sources.get(job.source) else {
                    continue;
                };
                let Some(result) = result else {
                    cache.release(&source.key, &job.sha, now);
                    continue;
                };
                match result {
                    Ok(report) => {
                        if report
                            .limit
                            .remaining
                            .is_some_and(|left| left < LOW_REMAINING)
                        {
                            let wait = report
                                .limit
                                .reset_at
                                .map(|reset| reset.saturating_sub(epoch_now()))
                                .map_or(LIMIT_PAUSE, Duration::from_secs);
                            let error = ApiError::RateLimited(
                                source.found.account.host(),
                                Some(wait.as_secs()),
                            );
                            cache.block(
                                &source.account,
                                now + wait,
                                pause_reason(&error, wait.as_secs()),
                            );
                        }
                        let state = hosting::aggregate(&report.checks);
                        let url = Some(hosting::commit_page(&source.found, &job.sha));
                        cache.store(
                            &source.key,
                            CiStatus {
                                sha: job.sha,
                                state,
                                checks: report.checks,
                                url,
                                note: report.note,
                            },
                            now,
                        );
                    }
                    Err(error) if missing(&error) => cache.missing(&source.key, &job.sha, now),
                    Err(error) => {
                        if let Some(wait) = wait_for(&error) {
                            tracing::info!(%error, "CI status paused for an account");
                            cache.block(
                                &source.account,
                                now + wait,
                                pause_reason(&error, wait.as_secs()),
                            );
                            cache.release(&source.key, &job.sha, now);
                        } else {
                            tracing::info!(%error, "CI status request failed");
                            cache.failed(&source.key, &job.sha, now);
                        }
                    }
                }
            }
        }

        let now = Instant::now();
        let mut cache = self.ci.lock();
        let mut statuses = Vec::new();
        let mut next: Option<Duration> = more.then_some(SOON);
        let mut sooner = |wait: Duration| {
            next = Some(next.map_or(wait, |held| held.min(wait)));
        };
        for target in &targets {
            let Some(source) = by_remote
                .get(&target.remote)
                .and_then(|at| sources.get(*at))
            else {
                continue;
            };
            let sha = target.sha.to_lowercase();
            let note = cache.blocked(&source.account, now).map(str::to_owned);
            if let Some(until) = cache.blocked_until(&source.account, now) {
                sooner(until.saturating_duration_since(now));
            } else if let Some(wait) = cache.due_in(&source.key, &sha, now) {
                sooner(wait);
            }
            if let Some(status) = cache.lookup(&source.key, &sha, now).status {
                let mut status = with_note(status, note.as_deref());
                status.sha.clone_from(&target.sha);
                statuses.push(status);
            }
        }
        Ok(CiBatch {
            statuses,
            retry_ms: next.map(retry_ms),
        })
    }

    pub fn clear_ci_statuses(&self) {
        self.ci.lock().clear();
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
