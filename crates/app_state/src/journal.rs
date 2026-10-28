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
    /// Exit 1 with unmerged paths left: partial success, not a failure.
    pub stopped_on_conflicts: bool,
    /// The app asks the user itself (`GitOutput::answered`): no error notice.
    pub answered: bool,
}

impl From<&git_engine::GitOutput> for CommandNotice {
    fn from(entry: &git_engine::GitOutput) -> Self {
        Self {
            id: entry.id,
            repo: entry.repo.clone(),
            operation: entry.operation.clone(),
            severity: entry.severity,
            summary: entry.summary.clone(),
            stopped_on_conflicts: entry.stopped_on_conflicts,
            answered: entry.answered || crate::network::asks_sign_in(entry.credential_trouble),
        }
    }
}

/// The Output panel's list, reread on every command: what ran, not what it printed.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandRow {
    pub id: u32,
    pub repo: String,
    pub operation: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u32,
    #[specta(type = specta_typescript::Number)]
    pub started_at_ms: u64,
    pub warned: bool,
}

impl From<&git_engine::GitOutput> for CommandRow {
    fn from(entry: &git_engine::GitOutput) -> Self {
        Self {
            id: entry.id,
            repo: entry.repo.clone(),
            operation: entry.operation.clone(),
            command: entry.command.clone(),
            exit_code: entry.exit_code,
            duration_ms: entry.duration_ms,
            started_at_ms: entry.started_at_ms,
            warned: is_warning(entry),
        }
    }
}

/// The Output panel is a recent history, not an audit log; the cap keeps a long session
/// from holding every byte Git ever printed.
pub(crate) const JOURNAL_CAPACITY: usize = 100;

pub(crate) type CommandTap = Box<dyn Fn(&git_engine::GitOutput) + Send + Sync>;

/// Git reports mixed line endings, permissions and deprecated settings on `stderr` with
/// exit code 0. Nobody sees those unless we call them out.
#[must_use]
pub fn is_warning(entry: &git_engine::GitOutput) -> bool {
    entry.exit_code == Some(0) && entry.stderr.lines().any(warns)
}

fn warns(line: &str) -> bool {
    let line = line.trim_start();
    let line = line.strip_prefix("remote:").map_or(line, str::trim_start);
    line.get(..8)
        .is_some_and(|start| start.eq_ignore_ascii_case("warning:"))
}

fn is_problem(entry: &git_engine::GitOutput) -> bool {
    entry.exit_code != Some(0) || is_warning(entry)
}

/// The journal's rows, newest first like `command_log`.
#[must_use]
pub fn command_rows(log: &std::collections::VecDeque<git_engine::GitOutput>) -> Vec<CommandRow> {
    log.iter().rev().map(CommandRow::from).collect()
}

/// Copy log: newest first, each entry as a terminal would have shown it.
#[must_use]
pub fn command_log_text(
    log: &std::collections::VecDeque<git_engine::GitOutput>,
    ids: &[u32],
) -> String {
    log.iter()
        .rev()
        .filter(|entry| ids.contains(&entry.id))
        .map(entry_text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn entry_text(entry: &git_engine::GitOutput) -> String {
    let exit = entry
        .exit_code
        .map_or_else(|| "?".to_owned(), |code| code.to_string());
    let mut text = format!(
        "$ {}\nexit {exit} in {} ms",
        entry.command, entry.duration_ms
    );
    for stream in [&entry.stdout, &entry.stderr] {
        if !stream.trim().is_empty() {
            text.push('\n');
            text.push_str(stream);
        }
    }
    text
}

/// The journal is a ring: the oldest entry makes room for the newest. Free-standing so the
/// bound can be proven with a capacity of three instead of five hundred git processes.
pub fn record(
    log: &mut std::collections::VecDeque<git_engine::GitOutput>,
    capacity: usize,
    entry: git_engine::GitOutput,
) {
    log.retain_back(capacity.max(1) - 1);
    log.push_back(entry);
}

impl AppState {
    pub(crate) fn tap_commands(&self, tap: CommandTap) {
        if self.command_tap.set(tap).is_err() {
            tracing::error!("the commands of this state are tapped already");
        }
    }

    /// Newest first: the Output panel opens on what just happened.
    #[must_use]
    pub fn command_log(&self) -> Vec<git_engine::GitOutput> {
        self.journal.read().iter().rev().cloned().collect()
    }

    #[must_use]
    pub fn command_rows(&self) -> Vec<CommandRow> {
        command_rows(&self.journal.read())
    }

    #[must_use]
    pub fn command_log_text(&self, ids: &[u32]) -> String {
        command_log_text(&self.journal.read(), ids)
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
            .filter(|entry| is_problem(entry))
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
        let tap = Arc::clone(&self.command_tap);
        let events = self.events.clone();
        let watchers = Arc::clone(&self.watchers);
        let walks = self.walks.epoch();
        Arc::new(move |entry| {
            walks.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            if let Some(watcher) = watchers.read().get(&repo) {
                watcher.quiet_for(fs_watcher::DEFAULT_QUIET);
            }
            if let Some(tap) = tap.get() {
                tap(&entry);
            }
            let _ = events.send(AppEvent::CommandRecorded(CommandNotice::from(&entry)));
            record(&mut journal.write(), JOURNAL_CAPACITY, entry);
        })
    }
}
