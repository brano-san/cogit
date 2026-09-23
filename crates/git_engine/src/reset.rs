use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

/// The five modes of `git reset <commit>`: what happens to the index and the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
    Keep,
    Merge,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
            Self::Keep => "--keep",
            Self::Merge => "--merge",
        }
    }
}

impl RepoHandle {
    pub fn reset(&self, rev: &str, mode: ResetMode) -> Result<()> {
        let rev = rev.trim();
        if rev.is_empty() || rev.starts_with('-') {
            return Err(GitError::InvalidState(format!("cannot reset to '{rev}'")));
        }
        let target = format!("{rev}^{{commit}}");
        let oid = self
            .run_git_reading(&["rev-parse", "--verify", "--quiet", &target])?
            .stdout
            .trim()
            .to_owned();
        self.run_git(&["reset", mode.flag(), &oid]).map(drop)
    }

    /// Through `gix`: the graph menu asks on every right-click.
    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        let ancestor = self.resolve_commit(ancestor)?;
        let descendant = self.resolve_commit(descendant)?;
        if ancestor == descendant {
            return Ok(true);
        }
        Ok(self
            .repo
            .merge_base(ancestor, descendant)
            .is_ok_and(|base| base.detach() == ancestor))
    }

    pub fn files_between(&self, from: &str, to: &str) -> Result<Vec<crate::FileEntry>> {
        let before = self.tree_of(self.resolve_commit(from)?)?;
        let after = self.tree_of(self.resolve_commit(to)?)?;
        self.files_between_trees(&before, &after, crate::DEFAULT_SIMILARITY)
    }

    pub(crate) fn resolve_commit(&self, rev: &str) -> Result<gix::ObjectId> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let commit = id
            .object()
            .map_err(|err| GitError::Internal(format!("cannot read {rev}: {err}")))?
            .peel_to_commit()
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?;
        Ok(commit.id)
    }
}
