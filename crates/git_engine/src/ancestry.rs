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

    /// Local branches that are safe to delete after a pull: the remote deleted their
    /// upstream and HEAD already holds every commit on them (R-211). The checked-out
    /// branch and branches held by other worktrees are kept; `git branch -d` would refuse.
    pub fn merged_gone_branches(&self) -> Result<Vec<String>> {
        let branches = self.branches()?;
        let remote: std::collections::HashSet<&str> = branches
            .iter()
            .filter(|branch| branch.kind == crate::BranchKind::Remote)
            .map(|branch| branch.name.as_str())
            .collect();

        let mut names: Vec<String> = branches
            .iter()
            .filter(|branch| branch.kind == crate::BranchKind::Local && !branch.is_head)
            .filter(|branch| {
                branch
                    .upstream
                    .as_deref()
                    .is_some_and(|upstream| !remote.contains(upstream))
            })
            .filter(|branch| self.is_merged_into_head(&branch.oid).unwrap_or(false))
            .map(|branch| branch.name.clone())
            .collect();
        if names.is_empty() {
            return Ok(names);
        }

        let held: Vec<String> = self
            .worktrees()?
            .into_iter()
            .filter_map(|entry| entry.branch)
            .collect();
        names.retain(|name| !held.contains(name));
        Ok(names)
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
