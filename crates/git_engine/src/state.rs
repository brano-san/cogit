//! Variants are detected from marker files in `.git`; none of them may panic (INV-07).

use crate::{Head, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RepoState {
    Clean,
    DetachedHead { oid: String },
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
    // `git am` stopped on a patch that did not apply.
    ApplyingPatches,
    Empty,
    Bare,
}

impl RepoState {
    #[must_use]
    pub fn is_interrupted_operation(&self) -> bool {
        matches!(
            self,
            Self::Merging
                | Self::Rebasing
                | Self::CherryPicking
                | Self::Reverting
                | Self::Bisecting
                | Self::ApplyingPatches
        )
    }

    #[must_use]
    pub fn allows_commit(&self) -> bool {
        !matches!(self, Self::Bare | Self::Bisecting)
    }
}

impl RepoHandle {
    /// An interrupted operation outranks everything else: it is what the user has to
    /// deal with before anything else works.
    pub fn state(&self) -> Result<RepoState> {
        if self.is_bare() {
            return Ok(RepoState::Bare);
        }

        let git_dir = self.git_dir();
        let marker = |name: &str| git_dir.join(name).exists();

        // `git am` and the apply backend of rebase share `rebase-apply`; am leaves `applying`.
        if marker("rebase-apply/applying") {
            return Ok(RepoState::ApplyingPatches);
        }
        // Before MERGE_HEAD, as git's own status does: `rebase -r` stopped on a merge
        // commit leaves both, and only the rebase can be continued or aborted.
        if marker("rebase-merge") || marker("rebase-apply") {
            return Ok(RepoState::Rebasing);
        }
        if marker("MERGE_HEAD") {
            return Ok(RepoState::Merging);
        }
        if marker("CHERRY_PICK_HEAD") {
            return Ok(RepoState::CherryPicking);
        }
        if marker("REVERT_HEAD") {
            return Ok(RepoState::Reverting);
        }
        if marker("BISECT_LOG") {
            return Ok(RepoState::Bisecting);
        }

        Ok(match self.head()? {
            Head::Unborn { .. } => RepoState::Empty,
            Head::Detached { oid } => RepoState::DetachedHead { oid },
            Head::Branch { .. } => RepoState::Clean,
        })
    }

    /// The path of a stale `index.lock`, which blocks every write until it is removed.
    #[must_use]
    pub fn index_lock(&self) -> Option<String> {
        let lock = self.git_dir().join("index.lock");
        lock.exists()
            .then(|| lock.to_string_lossy().replace('\\', "/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_operations_offer_continue_and_abort() {
        assert!(RepoState::Merging.is_interrupted_operation());
        assert!(RepoState::Rebasing.is_interrupted_operation());
        assert!(RepoState::ApplyingPatches.is_interrupted_operation());
        assert!(!RepoState::Clean.is_interrupted_operation());
        assert!(!RepoState::Empty.is_interrupted_operation());
    }

    #[test]
    fn detached_head_still_allows_committing() {
        let state = RepoState::DetachedHead {
            oid: "4ec4813".to_owned(),
        };
        assert!(state.allows_commit());
        assert!(!state.is_interrupted_operation());
    }

    #[test]
    fn bare_repositories_cannot_be_committed_to() {
        assert!(!RepoState::Bare.allows_commit());
    }
}
