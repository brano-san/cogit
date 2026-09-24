use crate::{RepoHandle, Result};

impl RepoHandle {
    /// HEAD already contains `rev`. Through `gix`: asked on every selection change (R-210).
    pub fn is_merged_into_head(&self, rev: &str) -> Result<bool> {
        let target = self.resolve_commit(rev)?;
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

    /// Merged into HEAD, upstream deleted on the remote; not HEAD's nor another worktree's
    /// branch, which `git branch -d` would refuse (R-211).
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
}
