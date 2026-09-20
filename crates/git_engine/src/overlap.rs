use crate::{GitError, RepoHandle, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::ops::ControlFlow;

impl RepoHandle {
    /// Against the first parent, as `git log` does for a merge. Names only, not bytes.
    pub fn changed_paths(&self, rev: &str) -> Result<Vec<String>> {
        changed_paths_in(&self.repo, rev)
    }
}

/// Free-standing for rayon: `gix::Repository` is deliberately not `Sync`.
fn changed_paths_in(repo: &gix::Repository, rev: &str) -> Result<Vec<String>> {
    let commit = commit_at(repo, rev)?;
    let tree = commit
        .tree()
        .map_err(|err| GitError::Internal(format!("cannot read commit tree: {err}")))?;

    let parent_tree = match commit.parent_ids().next() {
        Some(id) => repo
            .find_commit(id.detach())
            .map_err(|err| GitError::Internal(format!("cannot read parent commit: {err}")))?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read parent tree: {err}")))?,
        None => repo.empty_tree(),
    };

    let mut paths = Vec::new();
    parent_tree
        .changes()
        .map_err(|err| GitError::Internal(format!("cannot start a tree diff: {err}")))?
        .options(|options| {
            options.track_path();
            // A rename collapses two paths into one entry; an overlap needs both.
            options.track_rewrites(None);
        })
        .for_each_to_obtain_tree(&tree, |change| {
            // `-r` in git recurses and reports blobs; a tree entry is not a path.
            if !change.entry_mode().is_tree() {
                paths.push(change.location().to_string().replace('\\', "/"));
            }
            Ok::<_, std::convert::Infallible>(ControlFlow::Continue(()))
        })
        .map_err(|err| GitError::Internal(format!("tree diff failed: {err}")))?;

    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn commit_at<'r>(repo: &'r gix::Repository, rev: &str) -> Result<gix::Commit<'r>> {
    let id = repo
        .rev_parse_single(rev)
        .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
    id.object()
        .map_err(|err| GitError::Internal(format!("cannot read {rev}: {err}")))?
        .try_into_commit()
        .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))
}

/// The table in doc/modules/M13-commit-overlap.md, so the two cannot drift.
const HEAVY_FRACTION: f32 = 1.0 / 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Overlap {
    None,
    Slight,
    Heavy,
    Same,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OverlapRow {
    pub oid: String,
    pub overlap: Overlap,
    pub is_base: bool,
    pub shared: Vec<String>,
    pub shared_total: u32,
}

const TOOLTIP_PATHS: usize = 10;

#[must_use]
pub fn shared_paths(base: &[String], other: &[String]) -> Vec<String> {
    base.iter()
        .filter(|path| other.contains(path))
        .cloned()
        .collect()
}

#[must_use]
pub fn overlap_of(base: &[String], other: &[String]) -> Overlap {
    if base.is_empty() || other.is_empty() {
        return Overlap::None;
    }
    let shared = shared_paths(base, other).len();
    if shared == 0 {
        return Overlap::None;
    }
    if shared == base.len() && shared == other.len() {
        return Overlap::Same;
    }
    #[allow(clippy::cast_precision_loss)]
    if shared as f32 / base.len() as f32 > HEAVY_FRACTION {
        Overlap::Heavy
    } else {
        Overlap::Slight
    }
}

impl RepoHandle {
    /// Only the visible window: the whole history is tree comparisons nobody looks at.
    pub fn overlap_window(&self, base: &str, window: &[String]) -> Result<Vec<OverlapRow>> {
        let base_paths = self.changed_paths(base)?;
        let shared_repo = self.repo.clone().into_sync();

        Ok(window
            .par_iter()
            .map(|oid| {
                // One unreadable commit must not cost the whole column.
                let paths =
                    changed_paths_in(&shared_repo.to_thread_local(), oid).unwrap_or_default();
                let shared = shared_paths(&base_paths, &paths);
                OverlapRow {
                    oid: oid.clone(),
                    overlap: if oid == base {
                        Overlap::Same
                    } else {
                        overlap_of(&base_paths, &paths)
                    },
                    is_base: oid == base,
                    shared_total: u32::try_from(shared.len()).unwrap_or(u32::MAX),
                    shared: shared.into_iter().take(TOOLTIP_PATHS).collect(),
                }
            })
            .collect())
    }
}
