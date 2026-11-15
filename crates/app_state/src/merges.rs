use crate::{AppState, RepoId};
use std::sync::atomic::Ordering;

impl AppState {
    /// Branches ▸ merged marks (F-949); a newer scan, of any repository, stops this one.
    pub(crate) fn local_branch_merges(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::BranchMerge>, git_engine::GitError> {
        let handle = self.handle(repo)?;
        let mine = self.merge_scan.fetch_add(1, Ordering::Relaxed) + 1;
        let cancelled = || self.merge_scan.load(Ordering::Relaxed) != mine;
        let started = std::time::Instant::now();
        let found = self.with_merge_cache(repo, |cache| handle.branch_merges(cache, &cancelled));
        tracing::info!(elapsed = ?started.elapsed(), cancelled = cancelled(), "branch merges");
        found
    }

    pub(crate) fn with_merge_cache<T>(
        &self,
        repo: RepoId,
        work: impl FnOnce(&mut git_engine::MergeCache) -> T,
    ) -> T {
        let mut cache = self.merges.lock().remove(&repo).unwrap_or_default();
        let answer = work(&mut cache);
        let mut merges = self.merges.lock();
        if self.repos.read().contains_key(&repo) {
            merges.entry(repo).or_default().absorb(cache);
        }
        answer
    }
}
