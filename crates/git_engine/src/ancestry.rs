use crate::{GitError, RepoHandle, Result};

impl RepoHandle {
    /// True when HEAD already contains `rev`, so merging it would change nothing.
    /// Through `gix`: the toolbar asks on every selection change (R-210).
    pub fn is_merged_into_head(&self, rev: &str) -> Result<bool> {
        let target = self.commit_id(rev)?;
        let Ok(head) = self.repo.head_id() else {
            return Ok(false);
        };
        let head = head.detach();
        if head == target {
            return Ok(true);
        }
        match self.repo.merge_base(target, head) {
            Ok(base) => Ok(base.detach() == target),
            Err(_) => Ok(false),
        }
    }

    fn commit_id(&self, rev: &str) -> Result<gix::ObjectId> {
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
