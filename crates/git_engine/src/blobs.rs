use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffSpec {
    CommitVsParent {
        oid: String,
    },
    CommitVsCommit {
        a: String,
        b: String,
    },
    WorkTreeVsIndex,
    IndexVsHead,
    /// A past version against the file on disk now: Compare with Working Tree.
    CommitVsWorkTree {
        oid: String,
    },
}

/// Old and new contents of one path; `None` on a side means it is absent there.
pub type DiffSides = (Option<Vec<u8>>, Option<Vec<u8>>);

impl RepoHandle {
    /// `None` when the path is absent from that tree: an addition, not an empty file.
    pub fn blob_at(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let tree = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))?;

        self.blob_in(&tree, path)
    }

    /// The commit a gitlink points at on each side, or `None` when `path` is no submodule.
    /// The pointer is what changed, and all there is on one nobody checked out (R-139).
    pub fn submodule_pointer(
        &self,
        spec: &DiffSpec,
        path: &str,
    ) -> Result<Option<crate::SubmodulePointer>> {
        let (recorded, previous) = match spec {
            DiffSpec::CommitVsParent { oid } => (
                self.gitlink_at(oid, path),
                self.first_parent(oid)?
                    .and_then(|parent| self.gitlink_at(&parent, path)),
            ),
            DiffSpec::CommitVsCommit { a, b } => {
                (self.gitlink_at(b, path), self.gitlink_at(a, path))
            }
            DiffSpec::WorkTreeVsIndex => {
                let staged = self.gitlink_in_index(path);
                (self.checked_out_commit(path).or(staged.clone()), staged)
            }
            DiffSpec::IndexVsHead => (self.gitlink_in_index(path), self.gitlink_at("HEAD", path)),
            DiffSpec::CommitVsWorkTree { oid } => {
                (self.checked_out_commit(path), self.gitlink_at(oid, path))
            }
        };
        let Some(recorded) = recorded.or_else(|| previous.clone()) else {
            return Ok(None);
        };

        Ok(Some(crate::SubmodulePointer {
            checked_out: self.root().join(path).join(".git").exists()
                || self.root().join(path).join("HEAD").exists(),
            previous: previous.filter(|before| *before != recorded),
            recorded,
        }))
    }

    fn gitlink_in_index(&self, path: &str) -> Option<String> {
        let index = self.repo.index_or_empty().ok()?;
        let entry = index.entry_by_path(path.into())?;
        entry.mode.is_submodule().then(|| entry.id.to_string())
    }

    /// What the submodule's own HEAD is on: the working-tree side of its pointer.
    fn checked_out_commit(&self, path: &str) -> Option<String> {
        match RepoHandle::open_exact(&self.root().join(path))
            .ok()?
            .head()
            .ok()?
        {
            crate::Head::Branch { oid, .. } | crate::Head::Detached { oid } => Some(oid),
            crate::Head::Unborn { .. } => None,
        }
    }

    /// The object id a tree records for `path` when the entry is a gitlink.
    fn gitlink_at(&self, rev: &str, path: &str) -> Option<String> {
        let id = self.repo.rev_parse_single(rev).ok()?;
        let tree = self.repo.find_commit(id.detach()).ok()?.tree().ok()?;
        let entry = tree.lookup_entry_by_path(path).ok()??;
        if entry.mode().is_commit() {
            Some(entry.object_id().to_string())
        } else {
            None
        }
    }

    pub fn diff_sides(&self, spec: &DiffSpec, path: &str) -> Result<DiffSides> {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let new = self.blob_at(oid, path)?;
                let parent = self.first_parent(oid)?;
                let old = match parent {
                    Some(parent) => self.blob_at(&parent, path)?,
                    None => None,
                };
                Ok((old, new))
            }
            DiffSpec::CommitVsCommit { a, b } => {
                Ok((self.blob_at(a, path)?, self.blob_at(b, path)?))
            }
            DiffSpec::WorkTreeVsIndex => Ok((self.blob_in_index(path)?, self.blob_on_disk(path)?)),
            DiffSpec::IndexVsHead => {
                let old = match self.head()? {
                    crate::Head::Unborn { .. } => None,
                    _ => self.blob_at("HEAD", path)?,
                };
                Ok((old, self.blob_in_index(path)?))
            }
            DiffSpec::CommitVsWorkTree { oid } => {
                Ok((self.blob_at(oid, path)?, self.blob_on_disk(path)?))
            }
        }
    }

    /// What `diff_sides` would read, by size alone: a side too large to show is never read.
    pub fn side_sizes(&self, spec: &DiffSpec, path: &str) -> Result<(Option<u64>, Option<u64>)> {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let new = self.size_at(oid, path)?;
                let old = match self.first_parent(oid)? {
                    Some(parent) => self.size_at(&parent, path)?,
                    None => None,
                };
                Ok((old, new))
            }
            DiffSpec::CommitVsCommit { a, b } => {
                Ok((self.size_at(a, path)?, self.size_at(b, path)?))
            }
            DiffSpec::WorkTreeVsIndex => Ok((self.size_in_index(path)?, self.size_on_disk(path)?)),
            DiffSpec::IndexVsHead => {
                let old = match self.head()? {
                    crate::Head::Unborn { .. } => None,
                    _ => self.size_at("HEAD", path)?,
                };
                Ok((old, self.size_in_index(path)?))
            }
            DiffSpec::CommitVsWorkTree { oid } => {
                Ok((self.size_at(oid, path)?, self.size_on_disk(path)?))
            }
        }
    }

    fn size_at(&self, rev: &str, path: &str) -> Result<Option<u64>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let tree = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))?;
        let Some(entry) = tree
            .lookup_entry_by_path(path)
            .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?
        else {
            return Ok(None);
        };
        if !entry.mode().is_blob() {
            return Ok(None);
        }
        self.blob_size(entry.object_id(), path).map(Some)
    }

    fn size_in_index(&self, path: &str) -> Result<Option<u64>> {
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        match index.entry_by_path(path.into()) {
            Some(entry) if !entry.mode.is_submodule() => self.blob_size(entry.id, path).map(Some),
            _ => Ok(None),
        }
    }

    fn blob_size(&self, id: gix::ObjectId, path: &str) -> Result<u64> {
        self.repo
            .find_header(id)
            .map(|header| header.size())
            .map_err(|err| GitError::Internal(format!("cannot read {path}: {err}")))
    }

    fn size_on_disk(&self, path: &str) -> Result<Option<u64>> {
        if self.is_bare() {
            return Ok(None);
        }
        match std::fs::symlink_metadata(self.root().join(path)) {
            Ok(meta) if meta.is_dir() => Ok(None),
            Ok(meta) => Ok(Some(meta.len())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(GitError::Io(format!("cannot read {path}: {err}"))),
        }
    }

    /// The sides as git's own diff compares them, for a patch cut from them: the working
    /// file through the clean filters (eol, autocrlf, drivers), as `git add` would store it.
    /// `git apply` cleans the working file the same way before it patches it.
    pub fn patch_sides(&self, spec: &DiffSpec, path: &str) -> Result<DiffSides> {
        let (old, new) = self.diff_sides(spec, path)?;
        let on_disk = matches!(
            spec,
            DiffSpec::WorkTreeVsIndex | DiffSpec::CommitVsWorkTree { .. }
        );
        match new {
            Some(bytes) if on_disk && !is_symlink(&self.root().join(path)) => {
                Ok((old, Some(self.cleaned(path, bytes)?)))
            }
            new => Ok((old, new)),
        }
    }

    fn cleaned(&self, path: &str, bytes: Vec<u8>) -> Result<Vec<u8>> {
        use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
        use std::io::Read as _;

        let failed = |err: &dyn std::fmt::Display| {
            GitError::Internal(format!("cannot clean {path} as git add would: {err}"))
        };
        let (mut pipeline, index) = self.repo.filter_pipeline(None).map_err(|e| failed(&e))?;
        let outcome = pipeline
            .convert_to_git(bytes.as_slice(), std::path::Path::new(path), &index)
            .map_err(|e| failed(&e))?;
        let cleaned = match outcome {
            ToGitOutcome::Unchanged(_) => None,
            ToGitOutcome::Buffer(buffer) => Some(buffer.to_vec()),
            ToGitOutcome::Process(mut stream) => {
                let mut out = Vec::new();
                stream.read_to_end(&mut out).map_err(|e| failed(&e))?;
                Some(out)
            }
        };
        Ok(cleaned.unwrap_or(bytes))
    }

    pub(crate) fn blob_in_index(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let Some(entry) = index.entry_by_path(path.into()) else {
            return Ok(None);
        };
        // A gitlink names a commit of the submodule, which this object database never has.
        if entry.mode.is_submodule() {
            return Ok(None);
        }
        let object = self.repo.find_object(entry.id).map_err(|err| {
            GitError::Internal(format!("cannot read {path} from the index: {err}"))
        })?;
        Ok(Some(object.into_blob().data.clone()))
    }

    /// Bytes as they are on disk: the worktree side of a diff is not a Git object.
    ///
    /// Only a missing file, or a directory (a submodule), is "nothing here". A file that
    /// exists and cannot be read is an error: shown as absent it looks deleted. A symbolic
    /// link is its target path, as git records it, never the file it points at.
    pub(crate) fn blob_on_disk(&self, path: &str) -> Result<Option<Vec<u8>>> {
        if self.is_bare() {
            return Ok(None);
        }
        let file = self.root().join(path);
        if is_symlink(&file) {
            let target = std::fs::read_link(&file)
                .map_err(|err| GitError::Io(format!("cannot read the link {path}: {err}")))?;
            let target = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(target));
            return Ok(Some(target.into_owned().into()));
        }
        match std::fs::read(&file) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound || file.is_dir() => Ok(None),
            Err(err) => Err(GitError::Io(format!("cannot read {path}: {err}"))),
        }
    }

    pub(crate) fn first_parent(&self, rev: &str) -> Result<Option<String>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        let commit = self
            .repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))?;
        Ok(commit.parent_ids().next().map(|id| id.to_string()))
    }

    fn blob_in(&self, tree: &gix::Tree<'_>, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(entry) = tree
            .lookup_entry_by_path(path)
            .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?
        else {
            return Ok(None);
        };
        if !entry.mode().is_blob() {
            return Ok(None);
        }

        let object = entry
            .object()
            .map_err(|err| GitError::Internal(format!("cannot read {path}: {err}")))?;
        Ok(Some(object.into_blob().data.clone()))
    }
}

pub(crate) fn is_symlink(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}
