//! Only the repository the panels show is watched: Windows refuses to rename a folder
//! while a watch handle is open anywhere below it (R-351).

use crate::{AppEvent, AppState, RepoId, RepoOverview};
use std::path::Path;
use std::sync::Arc;

impl AppState {
    /// What changed while it was not watched is read afresh: its row here, its graph by
    /// the refs print (R-300), the rest by the open that brought it back.
    pub fn show_repository(&self, repo: Option<RepoId>) {
        let started = std::time::Instant::now();
        let left: Vec<fs_watcher::RepoWatcher> = {
            let mut watchers = self.watchers.write();
            *self.shown.lock() = repo;
            let others: Vec<RepoId> = watchers
                .keys()
                .copied()
                .filter(|id| Some(*id) != repo)
                .collect();
            others.iter().filter_map(|id| watchers.remove(id)).collect()
        };
        let stopped = left.len();
        // Outside the lock: dropping one joins the thread that delivers its events (R-126).
        drop(left);
        let watched_again = repo.is_some_and(|repo| self.watch_again(repo));
        tracing::info!(
            repo = repo.map(|repo| repo.0),
            stopped,
            watched_again,
            elapsed_ms = started.elapsed().as_millis(),
            "showing a repository"
        );
    }

    pub(crate) fn is_shown(&self, repo: RepoId) -> bool {
        *self.shown.lock() == Some(repo)
    }

    fn watch_again(&self, repo: RepoId) -> bool {
        if self.watchers.read().contains_key(&repo) {
            return false;
        }
        let handle = match self.handle(repo) {
            Ok(handle) => handle,
            Err(err) => {
                tracing::error!(error = ?err, context = "watching the repository shown");
                return false;
            }
        };
        self.start_watching(repo, handle.root(), handle.git_dir(), handle.common_dir());
        // Once the watcher is up, a change forgets the row by itself.
        self.forget_row(repo);
        true
    }

    /// With no watcher nothing says the row of a repository left behind went stale; its
    /// pulse, read from the disk, does. One that agrees keeps the row: a row costs a status.
    #[must_use]
    pub fn pulse(&self, root: &Path) -> git_engine::RepoPulse {
        let pulse = crate::repo_rows::pulse(root);
        let Some(repo) = self.find_by_root(root) else {
            return pulse;
        };
        if self.watchers.read().contains_key(&repo) {
            return pulse;
        }
        let Some(row) = self.cached_rows.read().rows.get(&repo).cloned() else {
            return pulse;
        };
        let stale = contradicts(&row, &pulse)
            || self
                .handle(repo)
                .and_then(|handle| handle.state())
                .is_ok_and(|state| state != row.state);
        if stale {
            self.forget_row(repo);
        }
        pulse
    }
}

/// The pulse skips untracked files, so a dirty row beside a clean pulse proves nothing.
fn contradicts(row: &RepoOverview, pulse: &git_engine::RepoPulse) -> bool {
    row.missing != pulse.missing
        || row.branch != pulse.branch
        || (row.ahead, row.behind) != (pulse.ahead, pulse.behind)
        || (pulse.dirty && !row.dirty)
}

pub(crate) struct Quiet<'a> {
    state: &'a AppState,
    repo: RepoId,
    /// The whole mutation, not a timer per git process (R-445).
    _held: Option<fs_watcher::QuietHold>,
}

impl Drop for Quiet<'_> {
    fn drop(&mut self) {
        self.state.silence(self.repo);
    }
}

impl AppState {
    /// A repository is watched once, and only while it is shown; reopening the same path
    /// must not stack watchers.
    pub(crate) fn start_watching(
        &self,
        repo: RepoId,
        root: &Path,
        git_dir: &Path,
        common_dir: &Path,
    ) {
        if self.watchers.read().contains_key(&repo) || !self.is_shown(repo) {
            return;
        }
        let events = self.events.clone();
        let rows = Arc::clone(&self.cached_rows);
        match fs_watcher::RepoWatcher::start(root, git_dir, common_dir, move |change| {
            rows.write().forget(repo);
            let _ = events.send(AppEvent::RepoChanged {
                repo,
                kind: change.kind,
            });
        }) {
            Ok(watcher) => {
                // Checked again under the lock: a second open of the same repository, a
                // close or a switch may have finished while this watcher was starting.
                let mut watchers = self.watchers.write();
                if watchers.contains_key(&repo)
                    || !self.repos.read().contains_key(&repo)
                    || !self.is_shown(repo)
                {
                    drop(watchers);
                    drop(watcher);
                } else {
                    watchers.insert(repo, watcher);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, repo = repo.0, "cannot watch the repository");
            }
        }
    }

    /// Called before every mutation: the UI reloads itself afterwards, so reacting to our
    /// own writes only makes it reload twice (doc/12-risks.md, R-25).
    /// Our own writes are the one change the watcher must not report: the UI reloads
    /// itself after a mutation. The watcher stays quiet while the guard lives and for one
    /// window after it drops, so a mutation that outlasts the window does not echo (R-445).
    #[must_use = "hold the guard until the mutation is done"]
    pub(crate) fn quiet(&self, repo: RepoId) -> Quiet<'_> {
        self.silence(repo);
        let held = self
            .watchers
            .read()
            .get(&repo)
            .map(fs_watcher::RepoWatcher::hold);
        Quiet {
            state: self,
            repo,
            _held: held,
        }
    }

    /// For a talk with a remote, minutes long, that writes at its end: a hold would keep
    /// the edits made meanwhile in an editor off the panels until it finished.
    #[must_use = "hold the guard until the mutation is done"]
    pub(crate) fn quiet_briefly(&self, repo: RepoId) -> Quiet<'_> {
        self.silence(repo);
        Quiet {
            state: self,
            repo,
            _held: None,
        }
    }

    fn silence(&self, repo: RepoId) {
        // The watcher will not report these writes, so the row has to be dropped here.
        self.forget_row(repo);
        if let Some(watcher) = self.watchers.read().get(&repo) {
            watcher.quiet_for(fs_watcher::DEFAULT_QUIET);
        }
    }

    /// Test seam: whether the watcher of `repo` is inside a quiet window right now.
    #[must_use]
    pub fn watcher_is_quiet(&self, repo: RepoId) -> bool {
        self.watchers
            .read()
            .get(&repo)
            .is_some_and(fs_watcher::RepoWatcher::is_quiet)
    }
}
