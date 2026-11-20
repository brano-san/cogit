use crate::{FileEntry, FileStatus, GitError, RepoHandle, Result, Signature};
use gix::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const REVIEW_COMMITS_SHOWN: usize = 2000;

const FALLBACK_REMOTE: &str = "origin";
const MAIN_NAMES: [&str; 2] = ["main", "master"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCommit {
    pub oid: String,
    pub summary: String,
    pub author: Signature,
    pub parents: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchReview {
    pub base: String,
    pub base_oid: String,
    pub branch: String,
    pub branch_oid: String,
    pub merge_base: String,
    pub merge_bases: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub commits: Vec<ReviewCommit>,
    pub is_head: bool,
}

impl RepoHandle {
    pub fn review_base(&self, branch: &str) -> Result<Option<String>> {
        let remote = self.review_remote(branch);
        let mut candidates = Vec::new();
        if let Some(default) = self.remote_default(&remote) {
            candidates.push(default);
        }
        candidates.extend(MAIN_NAMES.iter().map(|name| (*name).to_owned()));
        candidates.extend(MAIN_NAMES.iter().map(|name| format!("{remote}/{name}")));
        Ok(candidates
            .into_iter()
            .filter(|name| name != branch)
            .find(|name| self.commit_of(name).is_some()))
    }

    fn review_remote(&self, branch: &str) -> String {
        let local = |name: &str| {
            self.repo
                .branch_remote_name(name, gix::remote::Direction::Fetch)
                .map(|remote| remote.as_bstr().to_string())
        };
        let head = || {
            let name = self.repo.head_name().ok()??;
            local(&name.shorten().to_string())
        };
        local(branch)
            .or_else(head)
            .or_else(|| {
                let remotes: Vec<String> = self
                    .repo
                    .remote_names()
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                remotes
                    .iter()
                    .find(|name| *name == FALLBACK_REMOTE)
                    .or_else(|| remotes.first())
                    .cloned()
            })
            .unwrap_or_else(|| FALLBACK_REMOTE.to_owned())
    }

    fn remote_default(&self, remote: &str) -> Option<String> {
        let reference = self
            .repo
            .try_find_reference(format!("refs/remotes/{remote}/HEAD").as_str())
            .ok()??;
        match reference.target() {
            gix::refs::TargetRef::Symbolic(name) => Some(name.shorten().to_string()),
            gix::refs::TargetRef::Object(_) => None,
        }
    }

    pub fn branch_review(&self, base: &str, branch: &str) -> Result<BranchReview> {
        let base_oid = self.resolve_commit(base)?;
        let tip = self.resolve_commit(branch)?;
        let merge_bases: Vec<ObjectId> = self
            .repo
            .merge_bases_many(base_oid, &[tip])
            .map_err(|err| GitError::Internal(format!("cannot find the merge base: {err}")))?
            .into_iter()
            .map(gix::Id::detach)
            .collect();
        let Some(merge_base) = merge_bases.first().copied() else {
            return Err(GitError::InvalidState(format!(
                "{branch} and {base} have unrelated histories: no commit is common to both, so \
                 there is no merge base to review the branch against. Choose another base."
            )));
        };
        let own = self.only_reachable([tip], [base_oid]);
        let behind = self.only_reachable([base_oid], [tip]).len();
        let mailmap = self.mailmap();
        let commits = own
            .iter()
            .take(REVIEW_COMMITS_SHOWN)
            .map(|oid| self.review_commit(*oid, &mailmap))
            .collect::<Result<Vec<_>>>()?;
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(BranchReview {
            base: base.to_owned(),
            base_oid: base_oid.to_string(),
            branch: branch.to_owned(),
            branch_oid: tip.to_string(),
            merge_base: merge_base.to_string(),
            merge_bases: merge_bases.iter().map(ToString::to_string).collect(),
            ahead: count(own.len()),
            behind: count(behind),
            commits,
            is_head: self.is_head_branch(branch, tip),
        })
    }

    fn is_head_branch(&self, branch: &str, tip: ObjectId) -> bool {
        match self.head() {
            Ok(crate::Head::Branch { name, oid }) => {
                name == branch && ObjectId::from_hex(oid.as_bytes()).ok() == Some(tip)
            }
            _ => false,
        }
    }

    fn review_commit(&self, oid: ObjectId, mailmap: &crate::Mailmap) -> Result<ReviewCommit> {
        let commit = self
            .repo
            .find_commit(oid)
            .map_err(|err| GitError::Internal(format!("cannot read commit {oid}: {err}")))?;
        let decoded = commit
            .decode()
            .map_err(|err| GitError::Internal(format!("cannot decode commit {oid}: {err}")))?;
        let summary = decoded.message_summary().to_string();
        let author = decoded
            .author()
            .map_err(|err| GitError::Internal(format!("cannot read the author of {oid}: {err}")))?;
        let author = crate::commit::signature(author, mailmap);
        let parents = u32::try_from(decoded.parents().count()).unwrap_or(u32::MAX);
        Ok(ReviewCommit {
            oid: oid.to_string(),
            summary,
            author,
            parents,
        })
    }

    pub fn review_files(
        &self,
        merge_base: &str,
        branch: &str,
        worktree: bool,
    ) -> Result<Vec<FileEntry>> {
        let committed = self.files_between(merge_base, branch)?;
        if !worktree {
            return Ok(committed);
        }
        let tip = self.resolve_commit(branch)?;
        if self.repo.head_id().ok().map(gix::Id::detach) != Some(tip) {
            return Err(GitError::InvalidState(format!(
                "{branch} is not checked out, so it has no working tree to include"
            )));
        }
        let changed = self.worktree_files()?;
        let touched: BTreeMap<String, FileEntry> = changed
            .staged
            .into_iter()
            .chain(changed.unstaged)
            .filter(|file| counts_as_change(file.status))
            .flat_map(|file| {
                let old = file.old_path.clone().map(|old| (old, file.clone()));
                std::iter::once((file.path.clone(), file)).chain(old)
            })
            .collect();
        let base = merge_base.to_owned();
        let snapshot = self.diff_snapshot(&crate::DiffSpec::CommitVsWorkTree { oid: base })?;
        let mut files: BTreeMap<String, FileEntry> = committed
            .into_iter()
            .filter(|file| !touched.contains_key(&file.path))
            .map(|file| (file.path.clone(), file))
            .collect();
        for (path, template) in &touched {
            if let Some(status) = against_disk(&snapshot, path)? {
                let mut entry = template.clone();
                entry.path.clone_from(path);
                entry.old_path = None;
                entry.similarity = None;
                entry.conflict = None;
                entry.status = status;
                files.insert(path.clone(), entry);
            }
        }
        Ok(files.into_values().collect())
    }
}

fn counts_as_change(status: FileStatus) -> bool {
    !matches!(
        status,
        FileStatus::Untracked
            | FileStatus::Ignored
            | FileStatus::Unchanged
            | FileStatus::AssumeUnchanged
            | FileStatus::Skipped
            | FileStatus::Sparse
    )
}

fn against_disk(snapshot: &crate::DiffSnapshot<'_>, path: &str) -> Result<Option<FileStatus>> {
    let (old_size, new_size) = snapshot.sizes(path, path)?;
    Ok(match (old_size, new_size) {
        (None, None) => None,
        (None, Some(_)) => Some(FileStatus::Added),
        (Some(_), None) => Some(FileStatus::Deleted),
        (Some(_), Some(_)) => {
            let (old, new) = snapshot.ids(path, path)?;
            (old.is_none() || new.is_none() || old != new).then_some(FileStatus::Modified)
        }
    })
}
