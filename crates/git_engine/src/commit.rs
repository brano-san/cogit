use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::ops::ControlFlow;

const SECONDS_PER_MINUTE: i32 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub tz_offset_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub oid: String,
    pub parents: Vec<String>,
    pub summary: String,
    pub body: String,
    pub author: Signature,
    pub committer: Signature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
}

impl RepoHandle {
    pub fn commit_details(&self, rev: &str) -> Result<CommitDetails> {
        let commit = self.find_commit(rev)?;

        let message = commit
            .message()
            .map_err(|err| GitError::Internal(format!("cannot read commit message: {err}")))?;
        let author = commit
            .author()
            .map_err(|err| GitError::Internal(format!("cannot read commit author: {err}")))?;
        let committer = commit
            .committer()
            .map_err(|err| GitError::Internal(format!("cannot read committer: {err}")))?;

        Ok(CommitDetails {
            oid: commit.id().to_string(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            summary: message.summary().to_string(),
            body: message
                .body()
                .map(|body| body.to_string().trim_end().to_owned())
                .unwrap_or_default(),
            author: signature(author),
            committer: signature(committer),
        })
    }

    pub fn commit_files(&self, rev: &str) -> Result<Vec<FileEntry>> {
        let commit = self.find_commit(rev)?;
        let tree = commit
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read commit tree: {err}")))?;

        let parent_tree = match commit.parent_ids().next() {
            Some(id) => self
                .repo
                .find_commit(id.detach())
                .map_err(|err| GitError::Internal(format!("cannot read parent commit: {err}")))?
                .tree()
                .map_err(|err| GitError::Internal(format!("cannot read parent tree: {err}")))?,
            None => self.repo.empty_tree(),
        };

        let mut files = Vec::new();
        parent_tree
            .changes()
            .map_err(|err| GitError::Internal(format!("cannot start a tree diff: {err}")))?
            .options(|options| {
                options.track_path();
                options.track_rewrites(Some(gix::diff::Rewrites::default()));
            })
            .for_each_to_obtain_tree(&tree, |change| {
                if let Some(entry) = to_entry(&change) {
                    files.push(entry);
                }
                Ok::<_, std::convert::Infallible>(ControlFlow::Continue(()))
            })
            .map_err(|err| GitError::Internal(format!("tree diff failed: {err}")))?;

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    fn find_commit(&self, rev: &str) -> Result<gix::Commit<'_>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        id.object()
            .map_err(|err| GitError::Internal(format!("cannot read {rev}: {err}")))?
            .try_into_commit()
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))
    }
}

fn signature(sig: gix::actor::SignatureRef<'_>) -> Signature {
    let time = sig.time().unwrap_or_default();
    Signature {
        name: sig.name.to_string(),
        email: sig.email.to_string(),
        timestamp: time.seconds,
        tz_offset_minutes: time.offset / SECONDS_PER_MINUTE,
    }
}

fn to_entry(change: &gix::object::tree::diff::Change<'_, '_, '_>) -> Option<FileEntry> {
    use gix::object::tree::diff::Change;

    let (path, old_path, status) = match change {
        Change::Addition {
            location,
            entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            (location, None, FileStatus::Added)
        }
        Change::Deletion {
            location,
            entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            (location, None, FileStatus::Deleted)
        }
        Change::Modification {
            location,
            entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            (location, None, FileStatus::Modified)
        }
        Change::Rewrite {
            location,
            source_location,
            copy,
            ..
        } => (
            location,
            Some(source_location.to_string()),
            if *copy {
                FileStatus::Copied
            } else {
                FileStatus::Renamed
            },
        ),
    };

    Some(FileEntry {
        path: path.to_string(),
        old_path,
        status,
    })
}
