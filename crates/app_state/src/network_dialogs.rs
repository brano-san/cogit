//! The Pull and Push dialogs' operations: the same bookkeeping as the toolbar's own
//! Pull and Push, with the dialog's options added.

use crate::{AppState, RepoId};
use git_engine::{
    FetchOptions, GitError, NetworkDefaults, NetworkStop, NotesFetch, PullOptions, PushOptions,
    PushOutcome, PushPreview,
};

impl AppState {
    /// Fetch Only.
    pub fn fetch_with(
        &self,
        repo: RepoId,
        remote: &str,
        options: FetchOptions,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<NotesFetch, GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.fetch_options(remote, options, |url| self.token_for(url), on_line)
    }

    /// A branch that tracks nothing has nothing to merge: the dialog's Pull only fetches,
    /// as the toolbar's does (R-552).
    pub fn pull_with(
        &self,
        repo: RepoId,
        remote: &str,
        options: PullOptions,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<NotesFetch, GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        if handle.head_tracks_nothing() {
            return handle.fetch_options(remote, options.fetch, |url| self.token_for(url), on_line);
        }
        let known = handle
            .wants_new_submodules()
            .then(|| handle.submodule_paths());
        let before = handle.head()?;
        let result = handle.pull_options(remote, options, |url| self.token_for(url), on_line);
        self.record_move(
            repo,
            &handle,
            before,
            format!("Pull from {remote}"),
            &result,
        );
        let notes = result?;
        if let Some(known) = known {
            handle.init_submodules_added_since(&known)?;
        }
        Ok(notes)
    }

    pub fn push_with(
        &self,
        repo: RepoId,
        options: &PushOptions,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<PushOutcome, GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.push_options(options, |url| self.token_for(url), on_line)
    }

    /// Retry of the notes alone, after Fetch notes and merge.
    pub fn push_notes(
        &self,
        repo: RepoId,
        remote: &str,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<PushOutcome, GitError> {
        let _quiet = self.quiet_briefly(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        handle.push_notes(remote, |url| self.token_for(url), on_line)
    }

    pub fn merge_notes(&self, repo: RepoId, remote: &str, namespace: &str) -> Result<(), GitError> {
        self.handle(repo)?.merge_notes(remote, namespace)
    }

    pub fn push_preview(
        &self,
        repo: RepoId,
        local: &str,
        remote: &str,
        branch: &str,
        limit: usize,
    ) -> Result<PushPreview, GitError> {
        Ok(self
            .handle(repo)?
            .push_preview(local, remote, branch, limit))
    }

    pub fn network_defaults(&self, repo: RepoId) -> Result<NetworkDefaults, GitError> {
        self.handle(repo)?.network_defaults()
    }

    pub fn save_network_defaults(
        &self,
        repo: RepoId,
        defaults: &NetworkDefaults,
    ) -> Result<(), GitError> {
        self.handle(repo)?.save_network_defaults(defaults)
    }
}
