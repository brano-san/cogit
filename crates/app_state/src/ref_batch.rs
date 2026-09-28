//! Every ref under a folder of Branches, deleted in one queued step.

use crate::{AppState, RepoId};
use git_engine::{BranchDeletion, GitError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RefDeletionKind {
    Branch,
    RemoteBranch,
    Tag,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RefDeletion {
    pub kind: RefDeletionKind,
    /// The remote of `RemoteBranch` names, which are written `origin/topic`.
    pub remote: Option<String>,
    pub names: Vec<String>,
    /// Drops branches git calls not fully merged.
    pub force: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedDeletion {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FailedDeletion {
    pub name: String,
    pub error: GitError,
}

#[derive(Debug, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RefDeletionReport {
    pub deleted: Vec<String>,
    /// Kept by git; repeating with `force` drops them.
    pub not_fully_merged: Vec<String>,
    pub skipped: Vec<SkippedDeletion>,
    pub failed: Vec<FailedDeletion>,
}

/// `held`: each branch checked out somewhere, with the worktree's name unless it is this one.
/// Git refuses those, so they are left out up front and named in the report.
#[must_use]
pub fn split_deletable(
    names: &[String],
    held: &[(String, Option<String>)],
) -> (Vec<String>, Vec<SkippedDeletion>) {
    let mut deletable = Vec::new();
    let mut skipped = Vec::new();
    for name in names {
        match held.iter().find(|(branch, _)| branch == name) {
            None => deletable.push(name.clone()),
            Some((_, place)) => skipped.push(SkippedDeletion {
                name: name.clone(),
                reason: place.as_ref().map_or_else(
                    || "it is the current branch".to_owned(),
                    |worktree| format!("it is checked out in the worktree {worktree}"),
                ),
            }),
        }
    }
    (deletable, skipped)
}

impl AppState {
    pub fn delete_refs(
        &self,
        repo: RepoId,
        request: &RefDeletion,
    ) -> Result<RefDeletionReport, GitError> {
        let mut report = RefDeletionReport::default();
        let mut names = request.names.clone();
        if request.kind == RefDeletionKind::Branch {
            let held: Vec<(String, Option<String>)> = self
                .handle(repo)?
                .worktree_heads()?
                .into_iter()
                .filter_map(|entry| {
                    Some((entry.branch?, (!entry.is_current).then_some(entry.name)))
                })
                .collect();
            (names, report.skipped) = split_deletable(&names, &held);
        }
        let remote = request.remote.as_deref().unwrap_or("origin");
        for name in names {
            let outcome = match request.kind {
                RefDeletionKind::Branch => self
                    .delete_branch(repo, &name, request.force)
                    .map(|done| done == BranchDeletion::NotFullyMerged),
                RefDeletionKind::RemoteBranch => self
                    .delete_remote_branch(repo, remote, &name)
                    .map(|_| false),
                RefDeletionKind::Tag => self.delete_tag(repo, &name).map(|()| false),
            };
            match outcome {
                Ok(true) => report.not_fully_merged.push(name),
                Ok(false) => report.deleted.push(name),
                Err(error) => report.failed.push(FailedDeletion { name, error }),
            }
        }
        Ok(report)
    }
}
