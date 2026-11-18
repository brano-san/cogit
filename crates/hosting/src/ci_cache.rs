use crate::ci::{CiState, CiStatus};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const PENDING_FIRST: Duration = Duration::from_secs(15);
const PENDING_MAX: Duration = Duration::from_secs(300);
const NONE_FIRST: Duration = Duration::from_secs(120);
const NONE_MAX: Duration = Duration::from_secs(3600);
const FINISHED: Duration = Duration::from_secs(3600);
const MISSING: Duration = Duration::from_secs(1800);
const FAILED_FIRST: Duration = Duration::from_secs(300);
const FAILED_MAX: Duration = Duration::from_secs(1800);
const IN_FLIGHT: Duration = Duration::from_secs(60);
const CAP: usize = 4000;

fn doubled(first: Duration, max: Duration, round: u32) -> Duration {
    first.saturating_mul(1u32 << round.min(16)).min(max)
}

#[must_use]
pub fn ttl(state: CiState, round: u32) -> Duration {
    match state {
        CiState::Pending => doubled(PENDING_FIRST, PENDING_MAX, round),
        CiState::None => doubled(NONE_FIRST, NONE_MAX, round),
        CiState::Success | CiState::Failure | CiState::Neutral => FINISHED,
    }
}

#[derive(Debug, Clone)]
struct Entry {
    status: Option<CiStatus>,
    due: Instant,
    used: Instant,
    round: u32,
    failures: u32,
}

#[derive(Debug, Clone)]
struct Block {
    until: Instant,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    pub status: Option<CiStatus>,
    pub due: bool,
}

#[derive(Debug)]
pub struct CiCache {
    entries: HashMap<(String, String), Entry>,
    blocks: HashMap<String, Block>,
    cap: usize,
}

impl Default for CiCache {
    fn default() -> Self {
        Self::with_cap(CAP)
    }
}

fn key(source: &str, sha: &str) -> (String, String) {
    (source.to_owned(), sha.to_owned())
}

impl CiCache {
    #[must_use]
    pub fn with_cap(cap: usize) -> Self {
        Self {
            entries: HashMap::new(),
            blocks: HashMap::new(),
            cap: cap.max(1),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn lookup(&mut self, source: &str, sha: &str, now: Instant) -> Lookup {
        match self.entries.get_mut(&key(source, sha)) {
            Some(entry) => {
                entry.used = now;
                Lookup {
                    status: entry.status.clone(),
                    due: entry.due <= now,
                }
            }
            None => Lookup {
                status: None,
                due: true,
            },
        }
    }

    pub fn claim(&mut self, source: &str, sha: &str, now: Instant) -> bool {
        let entry = self.entries.entry(key(source, sha)).or_insert(Entry {
            status: None,
            due: now,
            used: now,
            round: 0,
            failures: 0,
        });
        if entry.due > now {
            return false;
        }
        entry.due = now + IN_FLIGHT;
        entry.used = now;
        self.trim();
        true
    }

    pub fn store(&mut self, source: &str, status: CiStatus, now: Instant) {
        let state = status.state;
        let sha = status.sha.clone();
        let entry = self.entries.entry(key(source, &sha)).or_insert(Entry {
            status: None,
            due: now,
            used: now,
            round: 0,
            failures: 0,
        });
        let same = entry
            .status
            .as_ref()
            .is_some_and(|held| held.state == state);
        entry.round = if same && !state.finished() {
            entry.round.saturating_add(1)
        } else {
            0
        };
        entry.due = now + ttl(state, entry.round);
        entry.status = Some(status);
        entry.used = now;
        entry.failures = 0;
        self.trim();
    }

    pub fn missing(&mut self, source: &str, sha: &str, now: Instant) {
        self.store(
            source,
            CiStatus {
                sha: sha.to_owned(),
                state: CiState::None,
                checks: Vec::new(),
                url: None,
                note: None,
            },
            now,
        );
        if let Some(entry) = self.entries.get_mut(&key(source, sha)) {
            entry.due = now + MISSING;
        }
    }

    pub fn failed(&mut self, source: &str, sha: &str, now: Instant) {
        if let Some(entry) = self.entries.get_mut(&key(source, sha)) {
            entry.due = now + doubled(FAILED_FIRST, FAILED_MAX, entry.failures);
            entry.failures = entry.failures.saturating_add(1);
        }
    }

    pub fn release(&mut self, source: &str, sha: &str, now: Instant) {
        if let Some(entry) = self.entries.get_mut(&key(source, sha)) {
            entry.due = now;
        }
    }

    pub fn block(&mut self, account: &str, until: Instant, reason: String) {
        self.blocks
            .insert(account.to_owned(), Block { until, reason });
    }

    #[must_use]
    pub fn blocked(&self, account: &str, now: Instant) -> Option<&str> {
        self.blocks
            .get(account)
            .filter(|block| block.until > now)
            .map(|block| block.reason.as_str())
    }

    #[must_use]
    pub fn blocked_until(&self, account: &str, now: Instant) -> Option<Instant> {
        self.blocks
            .get(account)
            .filter(|block| block.until > now)
            .map(|block| block.until)
    }

    #[must_use]
    pub fn due_in(&self, source: &str, sha: &str, now: Instant) -> Option<Duration> {
        let entry = self.entries.get(&key(source, sha))?;
        let unfinished = entry
            .status
            .as_ref()
            .is_none_or(|held| !held.state.finished());
        unfinished.then(|| entry.due.saturating_duration_since(now))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.blocks.clear();
    }

    fn trim(&mut self) {
        if self.entries.len() <= self.cap {
            return;
        }
        let mut ages: Vec<((String, String), Instant)> = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.used))
            .collect();
        ages.sort_by_key(|(_, used)| *used);
        let extra = self.entries.len() - self.cap;
        for (key, _) in ages.into_iter().take(extra) {
            self.entries.remove(&key);
        }
    }
}
