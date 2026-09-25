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

/// What `.gitattributes` says about showing paths as a diff, read once for a whole batch.
/// Unreadable attributes are logged and read as saying nothing.
pub struct DiffAttributes<'repo> {
    stack: Option<gix::AttributeStack<'repo>>,
}

impl std::fmt::Debug for DiffAttributes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffAttributes")
            .field("read", &self.stack.is_some())
            .finish()
    }
}

impl DiffAttributes<'_> {
    /// `-diff`, or `binary`, which unsets it: git shows the file as binary, whatever it holds.
    pub fn marks_binary(&mut self, path: &str) -> bool {
        let Some(stack) = self.stack.as_mut() else {
            return false;
        };
        let mut outcome = stack.selected_attribute_matches(["diff"]);
        match stack.at_entry(path, None) {
            Ok(platform) => platform.matching_attributes(&mut outcome),
            Err(err) => {
                tracing::error!(error = ?err, path, context = "reading .gitattributes for a diff");
                return false;
            }
        };
        outcome
            .iter_selected()
            .any(|found| found.assignment.state == gix::attrs::StateRef::Unset)
    }
}

impl RepoHandle {
    pub fn diff_attributes(&self) -> DiffAttributes<'_> {
        use gix::worktree::stack::state::attributes::Source;
        let source = if self.is_bare() {
            Source::IdMapping
        } else {
            Source::WorktreeThenIdMapping
        };
        let stack = self
            .repo
            .index_or_empty()
            .map_err(|err| err.to_string())
            .and_then(|index| {
                self.repo
                    .attributes_only(&index, source)
                    .map_err(|err| err.to_string())
            });
        match stack {
            Ok(stack) => DiffAttributes { stack: Some(stack) },
            Err(err) => {
                tracing::error!(error = %err, context = "reading .gitattributes for a diff");
                DiffAttributes { stack: None }
            }
        }
    }

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
            // A repository on disk is a submodule only where a side records its gitlink.
            DiffSpec::WorkTreeVsIndex => {
                let Some(staged) = self.gitlink_in_index(path) else {
                    return Ok(None);
                };
                (
                    self.checked_out_commit(path).or(Some(staged.clone())),
                    Some(staged),
                )
            }
            DiffSpec::IndexVsHead => (self.gitlink_in_index(path), self.gitlink_at("HEAD", path)),
            DiffSpec::CommitVsWorkTree { oid } => {
                let then = self.gitlink_at(oid, path);
                if then.is_none() && self.gitlink_in_index(path).is_none() {
                    return Ok(None);
                }
                (self.checked_out_commit(path), then)
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

    /// A folder at `path` in the working tree, and whether it holds a repository of its own.
    #[must_use]
    pub fn folder_on_disk(&self, path: &str) -> Option<bool> {
        let folder = self.root().join(path);
        (!self.is_bare() && folder.is_dir()).then(|| folder.join(".git").exists())
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
        let source = self.rename_source(spec, path)?;
        self.diff_sides_from(spec, source.as_deref().unwrap_or(path), path)
    }

    /// The sides with the old one read at `old_path`: the source of a rename or copy.
    pub fn diff_sides_from(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> Result<DiffSides> {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let new = self.blob_at(oid, path)?;
                let parent = self.first_parent(oid)?;
                let old = match parent {
                    Some(parent) => self.blob_at(&parent, old_path)?,
                    None => None,
                };
                Ok((old, new))
            }
            DiffSpec::CommitVsCommit { a, b } => {
                Ok((self.blob_at(a, old_path)?, self.blob_at(b, path)?))
            }
            DiffSpec::WorkTreeVsIndex => Ok((self.blob_in_index(path)?, self.worktree_side(path)?)),
            DiffSpec::IndexVsHead => {
                let old = match self.head()? {
                    crate::Head::Unborn { .. } => None,
                    _ => self.blob_at("HEAD", old_path)?,
                };
                Ok((old, self.blob_in_index(path)?))
            }
            DiffSpec::CommitVsWorkTree { oid } => {
                Ok((self.blob_at(oid, path)?, self.worktree_side(path)?))
            }
        }
    }

    /// What `diff_sides_from` would read, by size alone: a side too large to show is never read.
    pub fn side_sizes_from(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> Result<(Option<u64>, Option<u64>)> {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let new = self.size_at(oid, path)?;
                let old = match self.first_parent(oid)? {
                    Some(parent) => self.size_at(&parent, old_path)?,
                    None => None,
                };
                Ok((old, new))
            }
            DiffSpec::CommitVsCommit { a, b } => {
                Ok((self.size_at(a, old_path)?, self.size_at(b, path)?))
            }
            DiffSpec::WorkTreeVsIndex => Ok((self.size_in_index(path)?, self.worktree_size(path)?)),
            DiffSpec::IndexVsHead => {
                let old = match self.head()? {
                    crate::Head::Unborn { .. } => None,
                    _ => self.size_at("HEAD", old_path)?,
                };
                Ok((old, self.size_in_index(path)?))
            }
            DiffSpec::CommitVsWorkTree { oid } => {
                Ok((self.size_at(oid, path)?, self.worktree_size(path)?))
            }
        }
    }

    /// Where the old side keeps a file it has no `path` for: the source of the rename or
    /// copy the file list shows as `← old.txt`. Looked for only then, since finding it
    /// costs a tree diff with rename tracking.
    pub fn rename_source(&self, spec: &DiffSpec, path: &str) -> Result<Option<String>> {
        match spec {
            DiffSpec::CommitVsParent { oid } => match self.first_parent(oid)? {
                Some(parent) => self.rename_between(&parent, oid, path),
                None => Ok(None),
            },
            DiffSpec::CommitVsCommit { a, b } => self.rename_between(a, b, path),
            DiffSpec::IndexVsHead => self.staged_rename_source(path),
            DiffSpec::WorkTreeVsIndex | DiffSpec::CommitVsWorkTree { .. } => Ok(None),
        }
    }

    fn rename_between(&self, before: &str, after: &str, path: &str) -> Result<Option<String>> {
        let tree_of = |rev: &str| {
            self.find_commit(rev)?
                .tree()
                .map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))
        };
        let before = tree_of(before)?;
        if !matches!(before.lookup_entry_by_path(path), Ok(None)) {
            return Ok(None);
        }
        let files =
            self.files_between_trees(&before, &tree_of(after)?, crate::DEFAULT_SIMILARITY)?;
        Ok(files
            .into_iter()
            .find(|file| file.path == path)
            .and_then(|file| file.old_path))
    }

    /// The same rename tracking the Staged list gets from status.
    fn staged_rename_source(&self, path: &str) -> Result<Option<String>> {
        let Ok(tree) = self.repo.head_tree() else {
            return Ok(None);
        };
        if !matches!(tree.lookup_entry_by_path(path), Ok(None)) {
            return Ok(None);
        }
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let mut source = None;
        self.repo
            .tree_index_status(
                &tree.id,
                &index,
                None,
                gix::status::tree_index::TrackRenames::AsConfigured,
                |change, _, _| {
                    if let gix::diff::index::ChangeRef::Rewrite {
                        source_location,
                        location,
                        ..
                    } = change
                        && location.as_ref() == path
                    {
                        source = Some(source_location.to_string());
                    }
                    Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
                },
            )
            .map_err(|err| {
                GitError::Internal(format!("cannot compare the index with HEAD: {err}"))
            })?;
        Ok(source)
    }

    /// What `diff_sides` would read, by size alone.
    pub fn side_sizes(&self, spec: &DiffSpec, path: &str) -> Result<(Option<u64>, Option<u64>)> {
        let source = self.rename_source(spec, path)?;
        self.side_sizes_from(spec, source.as_deref().unwrap_or(path), path)
    }

    /// The working-tree side as git reads it. An entry marked skip-worktree (sparse
    /// checkout) or assume-unchanged stands for the file, whatever is on disk or is not:
    /// read from disk, a missing one looked deleted, and staging that deleted it.
    fn worktree_side(&self, path: &str) -> Result<Option<Vec<u8>>> {
        if self.index_stands_in(path)? {
            return self.blob_in_index(path);
        }
        self.blob_on_disk(path)
    }

    fn worktree_size(&self, path: &str) -> Result<Option<u64>> {
        if self.index_stands_in(path)? {
            return self.size_in_index(path);
        }
        self.size_on_disk(path)
    }

    fn index_stands_in(&self, path: &str) -> Result<bool> {
        use gix::index::entry::Flags;
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        Ok(index.entry_by_path(path.into()).is_some_and(|entry| {
            entry
                .flags
                .intersects(Flags::SKIP_WORKTREE | Flags::ASSUME_VALID)
        }))
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
