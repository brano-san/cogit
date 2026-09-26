//! Only the repository the panels show is watched: Windows refuses to rename a folder
//! while a watch handle is open anywhere below it (R-351).

use crate::{AppState, RepoId, RepoOverview};
use std::path::Path;

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
