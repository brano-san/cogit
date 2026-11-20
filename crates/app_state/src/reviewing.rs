use crate::{AppState, RepoId};
use git_engine::{DiffSpec, GitError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPath {
    pub path: String,
    #[serde(default)]
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LineCounts {
    pub path: String,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

impl AppState {
    pub(crate) fn local_review_base(
        &self,
        repo: RepoId,
        branch: &str,
    ) -> Result<Option<String>, GitError> {
        self.handle(repo)?.review_base(branch)
    }

    pub(crate) fn local_branch_review(
        &self,
        repo: RepoId,
        base: &str,
        branch: &str,
    ) -> Result<git_engine::BranchReview, GitError> {
        let started = std::time::Instant::now();
        let review = self.handle(repo)?.branch_review(base, branch)?;
        tracing::info!(elapsed = ?started.elapsed(), ahead = review.ahead, behind = review.behind, "branch review summary");
        Ok(review)
    }

    pub(crate) fn local_review_files(
        &self,
        repo: RepoId,
        merge_base: &str,
        branch: &str,
        worktree: bool,
    ) -> Result<Vec<git_engine::FileEntry>, GitError> {
        let started = std::time::Instant::now();
        let files = self
            .handle(repo)?
            .review_files(merge_base, branch, worktree)?;
        tracing::info!(elapsed = ?started.elapsed(), files = files.len(), worktree, "branch review files");
        Ok(files)
    }

    pub(crate) fn local_review_line_counts(
        &self,
        repo: RepoId,
        spec: &DiffSpec,
        files: &[ReviewPath],
    ) -> Result<Vec<LineCounts>, GitError> {
        let handle = self.handle(repo)?;
        let snapshot = handle.diff_snapshot(spec)?;
        files
            .iter()
            .map(|file| {
                let old_path = file.old_path.as_deref().unwrap_or(&file.path);
                let (old_size, new_size) = snapshot.sizes(old_path, &file.path)?;
                let size = old_size.unwrap_or(0).max(new_size.unwrap_or(0));
                let counts = if size >= diff_engine::MAX_TEXT_BYTES {
                    None
                } else {
                    match snapshot.sides(old_path, &file.path)? {
                        (None, None) => None,
                        (old, new) => diff_engine::line_counts(
                            old.as_deref().unwrap_or_default(),
                            new.as_deref().unwrap_or_default(),
                        ),
                    }
                };
                Ok(LineCounts {
                    path: file.path.clone(),
                    added: counts.map(|(added, _)| added),
                    removed: counts.map(|(_, removed)| removed),
                })
            })
            .collect()
    }
}
