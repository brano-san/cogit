//! The journal of git commands behind the Output panel: a ring of the newest.

use crate::{AppEvent, AppState, RepoId};
use serde::Serialize;
use std::sync::Arc;

/// What the UI needs to decide whether to interrupt the user. The output itself is
/// fetched by `id` from the journal, and only when somebody asks to see it.
#[derive(Debug, Clone, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandNotice {
    pub id: u32,
    pub repo: String,
    pub operation: String,
    pub severity: git_engine::Severity,
    pub summary: String,
}

impl From<&git_engine::GitOutput> for CommandNotice {
    fn from(entry: &git_engine::GitOutput) -> Self {
        Self {
            id: entry.id,
            repo: entry.repo.clone(),
            operation: entry.operation.clone(),
            severity: entry.severity,
            summary: entry.summary.clone(),
        }
    }
}

/// The Output panel is a recent history, not an audit log; the cap keeps a long session
/// from holding every byte Git ever printed.
pub(crate) const JOURNAL_CAPACITY: usize = 100;

/// Git reports mixed line endings, permissions and deprecated settings on `stderr` with
/// exit code 0. Nobody sees those unless we call them out.
#[must_use]
pub fn is_warning(entry: &git_engine::GitOutput) -> bool {
    entry.exit_code == Some(0) && !entry.stderr.trim().is_empty()
}

/// The journal is a ring: the oldest entry makes room for the newest. Free-standing so the
/// bound can be proven with a capacity of three instead of five hundred git processes.
pub fn record(
    log: &mut std::collections::VecDeque<git_engine::GitOutput>,
    capacity: usize,
    entry: git_engine::GitOutput,
) {
    while log.len() >= capacity.max(1) {
        log.pop_front();
    }
    log.push_back(entry);
}

impl AppState {
    /// Newest first: the Output panel opens on what just happened.
    #[must_use]
    pub fn command_log(&self) -> Vec<git_engine::GitOutput> {
        self.journal.read().iter().rev().cloned().collect()
    }

    /// One entry of the journal, by the number a notice carried. `None` once the ring
    /// has moved past it.
    #[must_use]
    pub fn command_outcome(&self, id: u32) -> Option<git_engine::GitOutput> {
        self.journal
            .read()
            .iter()
            .rev()
            .find(|entry| entry.id == id)
            .cloned()
    }

    /// Just the count: the indicator refreshes often and the entries can be a megabyte each.
    #[must_use]
    pub fn command_problems(&self) -> u32 {
        let count = self
            .journal
            .read()
            .iter()
            .filter(|entry| entry.exit_code != Some(0) || is_warning(entry))
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    pub fn clear_command_log(&self) {
        self.journal.write().clear();
    }

    /// Every git process of `repo` passes through here the moment it has exited. The quiet
    /// window is reopened from that moment, not only from the start and the end of the
    /// mutation: under load the process's last write can leave the debouncer before the
    /// mutation returns and the guard drops (R-197).
    pub(crate) fn command_sink(&self, repo: RepoId) -> git_engine::CommandSink {
        let journal = Arc::clone(&self.journal);
        let events = self.events.clone();
        let watchers = Arc::clone(&self.watchers);
        Arc::new(move |entry| {
            if let Some(watcher) = watchers.read().get(&repo) {
                watcher.quiet_for(fs_watcher::DEFAULT_QUIET);
            }
            let _ = events.send(AppEvent::CommandRecorded(CommandNotice::from(&entry)));
            record(&mut journal.write(), JOURNAL_CAPACITY, entry);
        })
    }
}
