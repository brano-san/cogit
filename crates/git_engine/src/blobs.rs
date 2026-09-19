use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffSpec {
    CommitVsParent { oid: String },
    CommitVsCommit { a: String, b: String },
}

/// Old and new contents of one path; `None` on a side means it is absent there.
pub type DiffSides = (Option<Vec<u8>>, Option<Vec<u8>>);

impl RepoHandle {
    /// `None` means the path is absent from that tree, which is how an addition or a
    /// deletion is told apart from an empty file.
    pub fn blob_at(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let tree = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))?;

        self.blob_in(&tree, path)
    }

    pub fn diff_sides(&self, spec: &DiffSpec, path: &str) -> Result<DiffSides> {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let new = self.blob_at(oid, path)?;
                let parent = self.first_parent(oid)?;
                let old = match parent {
                    Some(parent) => self.blob_at(&parent, path)?,
                    None => None,
                };
                Ok((old, new))
            }
            DiffSpec::CommitVsCommit { a, b } => {
                Ok((self.blob_at(a, path)?, self.blob_at(b, path)?))
            }
        }
    }

    fn first_parent(&self, rev: &str) -> Result<Option<String>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let commit = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?;
        Ok(commit.parent_ids().next().map(|id| id.to_string()))
    }

    fn blob_in(&self, tree: &gix::Tree<'_>, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(entry) = tree
            .lookup_entry_by_path(path)
            .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?
        else {
            return Ok(None);
        };
        if !entry.mode().is_blob() {
            return Ok(None);
        }

        let object = entry
            .object()
            .map_err(|err| GitError::Internal(format!("cannot read {path}: {err}")))?;
        Ok(Some(object.into_blob().data.clone()))
    }
}
