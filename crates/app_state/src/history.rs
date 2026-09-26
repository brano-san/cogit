//! Reads of history: commits and their files, blame, the history of a file, a line or a
//! fragment, content search and lookups.

use crate::{AppState, RepoId};

impl AppState {
    pub fn commit_details(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<git_engine::CommitDetails, git_engine::GitError> {
        self.handle(repo)?.commit_details(rev)
    }

    pub fn commit_files(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Vec<git_engine::FileEntry>, git_engine::GitError> {
        self.handle(repo)?.commit_files(rev)
    }

    pub fn tree_files(&self, repo: RepoId, rev: &str) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.tree_files(rev)
    }

    /// Looks inside files, handing matches over in batches as they are found.
    pub fn search_contents(
        &self,
        repo: RepoId,
        request: &git_engine::SearchRequest<'_>,
        cancelled: &dyn Fn() -> bool,
        on_batch: &mut dyn FnMut(Vec<git_engine::ContentMatch>),
    ) -> Result<(), git_engine::GitError> {
        self.handle(repo)?
            .search_contents(request, cancelled, on_batch)
    }

    pub fn find(
        &self,
        repo: RepoId,
        query: &str,
        limit: u32,
    ) -> Result<Vec<git_engine::Found>, git_engine::GitError> {
        self.handle(repo)?.find(query, limit as usize)
    }

    pub fn ref_dates(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::RefDate>, git_engine::GitError> {
        self.handle(repo)?.ref_dates()
    }

    pub fn lost_commits(
        &self,
        repo: RepoId,
        limit: u32,
    ) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> {
        let handle = self.handle(repo)?;
        // Taken out, not held: a slow walk must not block another repository's refresh.
        let mut cache = self.reachable.lock().remove(&repo).unwrap_or_default();
        let lost = handle.lost_commits_with(limit as usize, &mut cache);
        // Checked under the lock close clears it with, after unregistering.
        let mut reachable = self.reachable.lock();
        if self.repos.read().contains_key(&repo) {
            reachable.insert(repo, cache);
        }
        lost
    }

    pub fn blame(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
    ) -> Result<Vec<git_engine::BlameLine>, git_engine::GitError> {
        self.handle(repo)?.blame(path, rev)
    }

    pub fn line_history(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
        line: u32,
        limit: usize,
    ) -> Result<Vec<git_engine::LineVersion>, git_engine::GitError> {
        self.handle(repo)?.line_history(path, rev, line, limit)
    }

    pub fn file_revisions(
        &self,
        repo: RepoId,
        path: &str,
        rev: &str,
        limit: usize,
    ) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> {
        self.handle(repo)?.file_revisions(path, rev, limit)
    }

    /// Every commit that changed a fragment of a file, newest first.
    pub fn investigate(
        &self,
        repo: RepoId,
        path: &str,
        from: u32,
        to: u32,
        limit: u32,
    ) -> Result<Vec<git_engine::InvestigationStep>, git_engine::GitError> {
        self.handle(repo)?
            .investigate(path, from, to, limit as usize)
    }

    /// The file as it stood before a commit. Decoded lossily on purpose: a file the user
    /// cannot open at all is worse than one rendered oddly.
    pub fn file_before(
        &self,
        repo: RepoId,
        oid: &str,
        path: &str,
    ) -> Result<Option<String>, git_engine::GitError> {
        Ok(self
            .handle(repo)?
            .file_before(oid, path)?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }
}
