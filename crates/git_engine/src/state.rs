//! Variants are detected from marker files in `.git`; none of them may panic (INV-07).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind")]
pub enum RepoState {
    Clean,
    DetachedHead { oid: String },
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
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
        )
    }

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
