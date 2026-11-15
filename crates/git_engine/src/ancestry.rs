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
        if let Some(reached) = self.reaches_in_process(head, target) {
            return Ok(reached);
        }
        match self.repo.merge_base(target, head) {
            Ok(base) => Ok(base.detach() == target),
            Err(_) => Ok(false),
        }
    }

    /// Merged into HEAD (also by squash or rebase, F-949), upstream deleted on the remote;
    /// not HEAD's nor another worktree's branch, which `git branch -d` would refuse (R-211).
    pub fn merged_gone_branches(
        &self,
        cache: &mut crate::MergeCache,
    ) -> Result<Vec<crate::BranchMerge>> {
        let Ok(head) = self.repo.head_id() else {
            return Ok(Vec::new());
        };
        let branches = self.branches_without_divergence()?;
        let remote: std::collections::HashSet<&str> = branches
            .iter()
            .filter(|branch| branch.kind == crate::BranchKind::Remote)
            .map(|branch| branch.name.as_str())
            .collect();

        let gone: Vec<(&str, gix::ObjectId)> = branches
            .iter()
            .filter(|branch| branch.kind == crate::BranchKind::Local && !branch.is_head)
            .filter(|branch| {
                branch
                    .upstream
                    .as_deref()
                    .is_some_and(|upstream| !remote.contains(upstream))
            })
            .filter_map(|branch| {
                let tip = gix::ObjectId::from_hex(branch.oid.as_bytes()).ok()?;
                Some((branch.name.as_str(), tip))
            })
            .collect();
        if gone.is_empty() {
            return Ok(Vec::new());
        }
        let tips: Vec<gix::ObjectId> = gone.iter().map(|(_, tip)| *tip).collect();
        let kinds =
            self.merge_kinds(head.detach(), &tips, crate::MERGE_SCAN_CAP, cache, &|| {
                false
            })?;
        let mut merged: Vec<crate::BranchMerge> = gone
            .into_iter()
            .zip(kinds)
            .filter_map(|((name, tip), kind)| {
                let kind = kind.filter(|kind| kind.is_merged())?;
                Some(crate::BranchMerge {
                    name: name.to_owned(),
                    oid: tip.to_string(),
                    kind,
                })
            })
            .collect();
        if merged.is_empty() {
            return Ok(merged);
        }

        let held: Vec<String> = self
            .worktree_heads()?
            .into_iter()
            .filter_map(|entry| entry.branch)
            .collect();
        merged.retain(|branch| !held.contains(&branch.name));
        Ok(merged)
    }
}
