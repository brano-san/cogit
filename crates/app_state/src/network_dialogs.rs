//! The Pull and Push dialogs' operations: the same bookkeeping as the toolbar's own
//! Pull and Push, with the dialog's options added.

use crate::{AppState, RepoId, SignIn};
use git_engine::{
    FetchOptions, GitError, NetworkDefaults, NetworkStop, NotesFetch, PullOptions, Pulled,
    PushOptions, PushOutcome, PushPreview,
};

impl AppState {
    /// Fetch Only.
    pub fn fetch_with(
        &self,
        repo: RepoId,
        remote: &str,
        options: FetchOptions,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<NotesFetch, GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.fetch_options(remote, options, credential, on_line)
        })
    }

    /// A branch that tracks nothing has nothing to merge: the dialog's Pull only fetches,
    /// as the toolbar's does (R-552).
    pub fn pull_with(
        &self,
        repo: RepoId,
        remote: &str,
        options: PullOptions,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<Pulled, GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            if handle.head_tracks_nothing()
                || matches!(handle.head()?, git_engine::Head::Detached { .. })
            {
                let notes = handle.fetch_options(remote, options.fetch, credential, on_line)?;
                return Ok(Pulled {
                    notes,
                    ..Pulled::default()
                });
            }
            let known = handle
                .wants_new_submodules()
                .then(|| handle.submodule_paths());
            let before = handle.head()?;
            let result = handle.pull_options(remote, options, credential, on_line);
            self.record_move(
                repo,
                &handle,
                before,
                format!("Pull from {remote}"),
                &result,
            );
            let pulled = result?;
            if let Some(known) = known.filter(|_| !pulled.stash_needed) {
                handle.init_submodules_added_since(&known)?;
            }
            Ok(pulled)
        })
    }

    pub fn push_with(
        &self,
        repo: RepoId,
        options: &PushOptions,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<PushOutcome, GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.push_options(options, credential, on_line)
        })
    }

    pub fn push_dry_run(
        &self,
        repo: RepoId,
        options: &PushOptions,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<git_engine::GitOutput, GitError> {
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.push_dry_run(options, credential, on_line)
        })
    }

    /// Retry of the notes alone, after Fetch notes and merge.
    pub fn push_notes(
        &self,
        repo: RepoId,
        remote: &str,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<PushOutcome, GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.push_notes(remote, credential, on_line)
        })
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

    /// Tags the dialog's push would send. Kept out of the journal: a remote that cannot be
    /// asked is a line in the dialog, not a notification.
    pub fn push_tags_preview(
        &self,
        repo: RepoId,
        options: &PushOptions,
    ) -> Result<Vec<String>, GitError> {
        self.unjournaled_handle(repo)?
            .push_tags_preview(options, self.stored())
    }

    /// Whether the remote takes `--atomic`; `None` when it could not be asked. Unjournaled
    /// for the reason the tag count is.
    pub fn push_atomic_support(
        &self,
        repo: RepoId,
        options: &PushOptions,
    ) -> Result<Option<bool>, GitError> {
        self.unjournaled_handle(repo)?
            .push_atomic_support(options, self.stored())
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
