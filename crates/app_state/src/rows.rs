//! Rows of the Repositories tree, cached until the repository changes.

use crate::{AppState, OpenRepo, RepoId};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

/// Commits arrive with their lane placement so the UI never computes layout (INV-02).
/// One row of the repository tree: enough to draw it without opening every repository.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoOverview {
    pub repo: RepoId,
    pub name: String,
    pub root: String,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    /// The folder is gone. The row stays so the user can remove it on purpose (T3.7).
    pub missing: bool,
    /// An operation stopped half way, or a detached HEAD: the row labels it (#22).
    pub state: git_engine::RepoState,
}

/// Tree rows by repository. A row read while its repository changed is not kept: the read
/// began before the change and may describe the state before it.
#[derive(Debug, Default)]
pub(crate) struct RowCache {
    pub(crate) rows: HashMap<RepoId, RepoOverview>,
    /// Per repository: a change in one says nothing about a row of another.
    forgotten: HashMap<RepoId, u64>,
}

impl RowCache {
    /// What `keep` needs to tell a read that raced a change of `repo`.
    fn begin(&self, repo: RepoId) -> u64 {
        self.forgotten.get(&repo).copied().unwrap_or(0)
    }

    fn keep(&mut self, since: u64, row: RepoOverview) {
        if self.begin(row.repo) == since {
            self.rows.insert(row.repo, row);
        }
    }

    pub(crate) fn forget(&mut self, repo: RepoId) {
        self.rows.remove(&repo);
        *self.forgotten.entry(repo).or_default() += 1;
    }
}

impl AppState {
    /// Sorted by name so the tree does not reshuffle when a repository is reopened.
    #[must_use]
    pub fn overviews(&self) -> Vec<RepoOverview> {
        let mut rows: Vec<RepoOverview> = self
            .list()
            .into_iter()
            .filter(|open| open.listed)
            .map(|open| self.row_for(&open))
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Reading a row costs a `head`, a `branches` and a `status`; the panel asks for the
    /// whole tree on every refresh, and most rows have not moved since it last asked.
    fn row_for(&self, open: &OpenRepo) -> RepoOverview {
        let since = {
            let cache = self.cached_rows.read();
            if let Some(row) = cache.rows.get(&open.id) {
                return row.clone();
            }
            cache.begin(open.id)
        };
        let row = self.overview_of(open);
        self.rows_read.fetch_add(1, Ordering::Relaxed);
        self.cached_rows.write().keep(since, row.clone());
        row
    }

    fn overview_of(&self, open: &OpenRepo) -> RepoOverview {
        let mut row = RepoOverview {
            repo: open.id,
            name: open.display_name.clone(),
            root: open.root.to_string_lossy().replace('\\', "/"),
            branch: None,
            ahead: 0,
            behind: 0,
            dirty: false,
            missing: false,
            state: git_engine::RepoState::Clean,
        };

        let Ok(handle) = git_engine::RepoHandle::open_root(&open.root) else {
            row.missing = true;
            return row;
        };
        if let Ok(git_engine::Head::Branch { name, .. }) = handle.head() {
            row.branch = Some(name);
        }
        if let Ok(branches) = handle.branches()
            && let Some(current) = branches.into_iter().find(|b| b.is_head)
        {
            row.ahead = current.ahead;
            row.behind = current.behind;
        }
        if let Ok(status) = handle.status() {
            row.dirty = !status.is_clean();
        }
        if let Ok(state) = handle.state() {
            row.state = state;
        }
        row
    }

    /// Whatever this repository's row said is no longer true.
    pub fn forget_row(&self, repo: RepoId) {
        self.cached_rows.write().forget(repo);
    }

    /// How many rows were built from git rather than served from the last look.
    #[must_use]
    pub fn rows_read(&self) -> u32 {
        self.rows_read.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(repo: u32) -> RepoOverview {
        RepoOverview {
            repo: RepoId(repo),
            name: "a".into(),
            root: "/a".into(),
            branch: None,
            ahead: 0,
            behind: 0,
            dirty: false,
            missing: false,
            state: git_engine::RepoState::Clean,
        }
    }

    // The watcher dropped the row while the tree was reading it, and the read then put back
    // what it saw before the commit: the row stayed stale until the next change.
    #[test]
    fn a_row_read_across_a_change_is_not_kept() {
        let mut cache = RowCache::default();
        let since = cache.begin(RepoId(1));
        cache.forget(RepoId(1));
        cache.keep(since, row(1));
        assert!(cache.rows.is_empty());
    }

    // One counter for every repository: a change in a noisy one threw away rows of all the
    // others read meanwhile, and the next list read them again with a full status.
    #[test]
    fn a_change_in_another_repository_does_not_throw_the_row_away() {
        let mut cache = RowCache::default();
        let since = cache.begin(RepoId(1));
        cache.forget(RepoId(2));
        cache.keep(since, row(1));
        assert!(cache.rows.contains_key(&RepoId(1)));
    }

    #[test]
    fn a_row_read_undisturbed_is_kept() {
        let mut cache = RowCache::default();
        let since = cache.begin(RepoId(1));
        cache.keep(since, row(1));
        assert!(cache.rows.contains_key(&RepoId(1)));
    }
}
