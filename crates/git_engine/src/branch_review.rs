use crate::surgery::branch_of;
use crate::{FileEntry, FileStatus, GitError, RepoHandle, Result, Signature};
use gix::ObjectId;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeMap;

const FALLBACK_REMOTE: &str = "origin";
const MAIN_NAMES: [&str; 2] = ["master", "main"];

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
    pub merge_base_summary: String,
    pub merge_bases: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub commits: Vec<ReviewCommit>,
    pub is_head: bool,
}

impl RepoHandle {
    pub fn review_base(&self, branch: &str) -> Result<Option<String>> {
        let remotes = self.remotes()?;
        let remote = self.review_remote(branch, &remotes);
        let mut candidates = Vec::new();
        if let Some(default) = self.remote_default(&remote) {
            candidates.push(default);
        }
        candidates.extend(MAIN_NAMES.map(|name| format!("refs/heads/{name}")));
        candidates.extend(MAIN_NAMES.map(|name| format!("refs/remotes/{remote}/{name}")));
        if let Some(base) = candidates
            .into_iter()
            .find(|name| self.commit_of(name).is_some())
        {
            return Ok(Some(base));
        }
        let branches = self.branches_without_divergence()?;
        Ok(MAIN_NAMES.iter().find_map(|name| {
            branches
                .iter()
                .find(|branch| {
                    branch.kind == crate::BranchKind::Remote
                        && branch_of(&branch.full_name, &remotes) == *name
                })
                .map(|branch| branch.full_name.clone())
        }))
    }

    fn review_remote(&self, branch: &str, remotes: &[String]) -> String {
        if let Some(name) = branch.strip_prefix("refs/remotes/") {
            return name
                .strip_suffix(branch_of(branch, remotes))
                .and_then(|remote| remote.strip_suffix('/'))
                .unwrap_or(FALLBACK_REMOTE)
                .to_owned();
        }
        let name = branch.strip_prefix("refs/heads/").unwrap_or(branch);
        self.repo
            .branch_remote_name(name, gix::remote::Direction::Fetch)
            .map(|remote| remote.as_bstr().to_string())
            .filter(|remote| remote != ".")
            .unwrap_or_else(|| FALLBACK_REMOTE.to_owned())
    }

    fn remote_default(&self, remote: &str) -> Option<String> {
        let reference = self
            .repo
            .try_find_reference(format!("refs/remotes/{remote}/HEAD").as_str())
            .ok()??;
        match reference.target() {
            gix::refs::TargetRef::Symbolic(name) => Some(name.as_bstr().to_string()),
            gix::refs::TargetRef::Object(_) => None,
        }
    }

    pub fn branch_review(&self, base: &str, branch: &str) -> Result<BranchReview> {
        let base_oid = self.resolve_commit(base)?;
        let tip = self.resolve_commit(branch)?;
        let mut merge_bases: Vec<ObjectId> = self
            .repo
            .merge_bases_many(base_oid, &[tip])
            .map_err(|err| GitError::Internal(format!("cannot find the merge base: {err}")))?
            .into_iter()
            .map(gix::Id::detach)
            .collect();
        if merge_bases.len() > 1 {
            let mut dated = merge_bases
                .iter()
                .map(|&id| {
                    self.repo
                        .find_commit(id)
                        .and_then(|commit| commit.time())
                        .map(|time| (Reverse(time.seconds), id))
                        .map_err(|err| {
                            GitError::Internal(format!("cannot read merge base {id}: {err}"))
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            dated.sort_by_key(|&(date, _)| date);
            merge_bases = dated.into_iter().map(|(_, id)| id).collect();
        }
        let Some(merge_base) = merge_bases.first().copied() else {
            return Err(GitError::InvalidState(format!(
                "{} and {} have unrelated histories: no commit is common to both, so \
                 there is no merge base to review the branch against. Choose another base.",
                branch_name(branch),
                branch_name(base)
            )));
        };
        let own = self.only_reachable([tip], [base_oid]);
        let behind = self.only_reachable([base_oid], [tip]).len();
        let mailmap = self.mailmap();
        let commits = own
            .iter()
            .map(|oid| self.review_commit(*oid, &mailmap))
            .collect::<Result<Vec<_>>>()?;
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(BranchReview {
            base: base.to_owned(),
            base_oid: base_oid.to_string(),
            branch: branch.to_owned(),
            branch_oid: tip.to_string(),
            merge_base: merge_base.to_string(),
            merge_base_summary: self.review_commit(merge_base, &mailmap)?.summary,
            merge_bases: merge_bases.iter().map(ToString::to_string).collect(),
            ahead: count(own.len()),
            behind: count(behind),
            commits,
            is_head: !self.is_bare() && self.is_head_branch(branch, tip),
        })
    }

    fn is_head_branch(&self, branch: &str, tip: ObjectId) -> bool {
        if branch.starts_with("refs/remotes/") {
            return false;
        }
        match self.head() {
            Ok(crate::Head::Branch { name, oid }) => {
                name == branch_name(branch) && ObjectId::from_hex(oid.as_bytes()).ok() == Some(tip)
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
        if !worktree {
            return self.files_between(merge_base, branch);
        }
        let tip = self.resolve_commit(branch)?;
        if !self.is_head_branch(branch, tip) {
            return Err(GitError::InvalidState(format!(
                "{} is not checked out, so it has no working tree to include",
                branch_name(branch)
            )));
        }
        let committed = self.files_between(merge_base, &tip.to_string())?;
        let changed = self.worktree_files()?;
        let mut touched: BTreeMap<String, FileEntry> = changed
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
        let mut files = BTreeMap::new();
        for file in committed {
            let source_touched = file
                .old_path
                .as_ref()
                .is_some_and(|old| touched.contains_key(old));
            if source_touched || touched.contains_key(&file.path) {
                if file.status == FileStatus::Renamed
                    && let Some(old) = &file.old_path
                {
                    touched.entry(old.clone()).or_insert_with(|| file.clone());
                }
                touched.entry(file.path.clone()).or_insert(file);
            } else {
                files.insert(file.path.clone(), file);
            }
        }
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

fn branch_name(reference: &str) -> &str {
    reference
        .strip_prefix("refs/heads/")
        .or_else(|| reference.strip_prefix("refs/remotes/"))
        .unwrap_or(reference)
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
