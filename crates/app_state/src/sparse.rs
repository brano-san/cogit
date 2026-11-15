//! Repository ▸ Sparse Checkout (F-955); Undo sets the previous set back.

use crate::{AppState, Recovery, RepoId};
use git_engine::{GitError, NetworkStop, SparseChange, SparseFolder, SparsePreview, SparseState};

impl AppState {
    pub(crate) fn local_sparse_state(&self, repo: RepoId) -> Result<SparseState, GitError> {
        self.handle(repo)?.sparse_state()
    }

    pub(crate) fn local_sparse_folders(
        &self,
        repo: RepoId,
        dir: &str,
    ) -> Result<Vec<SparseFolder>, GitError> {
        self.handle(repo)?.sparse_folders(dir)
    }

    pub(crate) fn local_sparse_preview(
        &self,
        repo: RepoId,
        change: &SparseChange,
    ) -> Result<SparsePreview, GitError> {
        self.handle(repo)?.sparse_preview(change)
    }

    /// Journalled whenever the set changed, a canceled run included: git may have written
    /// the new set before it was stopped.
    pub(crate) fn local_sparse_apply(
        &self,
        repo: RepoId,
        change: &SparseChange,
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?.with_stop(stop.clone());
        let before = handle.sparse_state()?;
        let result = handle.sparse_apply(change);
        match handle.sparse_state() {
            Ok(after) if after != before => {
                self.record(repo, described(change), Recovery::Sparse { before });
            }
            Ok(_) => {}
            Err(err) => {
                tracing::error!(error = ?err, context = "sparse checkout unreadable after the change; not kept for Undo");
            }
        }
        result
    }
}

fn described(change: &SparseChange) -> String {
    match change {
        SparseChange::Disable => "Disable sparse checkout".to_owned(),
        SparseChange::Set { cone: true, set } => match set.as_slice() {
            [] => "Sparse checkout: top-level files only".to_owned(),
            [only] => format!("Sparse checkout: {only}"),
            many => format!("Sparse checkout: {} folders", many.len()),
        },
        SparseChange::Set { cone: false, set } => {
            format!("Sparse checkout: {} patterns", set.len())
        }
    }
}
