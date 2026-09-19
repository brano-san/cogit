//! Repository states that need their own banner in the UI.
//!
//! Each variant is detected from marker files inside `.git`. See
//! `doc/03-git-semantics.md` section 4 — every variant must be covered by a test
//! proving Cogit does not panic on it (INV-07).

use serde::Serialize;

/// What the repository is in the middle of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind")]
pub enum RepoState {
    /// Nothing in progress.
    Clean,
    /// HEAD points at a commit rather than a branch.
    DetachedHead { oid: String },
    /// `.git/MERGE_HEAD` exists.
    Merging,
    /// `.git/rebase-merge/` or `.git/rebase-apply/` exists.
    Rebasing,
    /// `.git/CHERRY_PICK_HEAD` exists.
    CherryPicking,
    /// `.git/REVERT_HEAD` exists.
    Reverting,
    /// `.git/BISECT_LOG` exists.
    Bisecting,
    /// Initialised but without any commit yet.
    Empty,
    /// No working tree.
    Bare,
}

impl RepoState {
    /// Whether the state offers `Continue` / `Abort` actions in the banner.
    #[must_use]
    pub fn is_interrupted_operation(&self) -> bool {
        matches!(
            self,
            Self::Merging
                | Self::Rebasing
                | Self::CherryPicking
                | Self::Reverting
                | Self::Bisecting
        )
    }

    /// Whether committing makes sense in this state.
    #[must_use]
    pub fn allows_commit(&self) -> bool {
        !matches!(self, Self::Bare | Self::Bisecting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_operations_offer_continue_and_abort() {
        assert!(RepoState::Merging.is_interrupted_operation());
        assert!(RepoState::Rebasing.is_interrupted_operation());
        assert!(!RepoState::Clean.is_interrupted_operation());
        assert!(!RepoState::Empty.is_interrupted_operation());
    }

    #[test]
    fn detached_head_still_allows_committing() {
        // Committing on a detached HEAD is legitimate; the banner warns, it does not block.
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
