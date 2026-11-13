use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

/// A working file past this is not read to name it: the same cap past which a diff does
/// not read a side at all (`diff_engine::MAX_IMAGE_BYTES`).
pub const MAX_HASHED_BYTES: u64 = 20 * 1024 * 1024;

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

/// What `.gitattributes` says about reading a path as lines (R-531).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffContent {
    /// Nothing: the bytes decide.
    Detect,
    /// `text`, or `diff` set: lines, whatever the bytes hold, as git diffs them.
    Text,
    /// By the attribute that says so: `binary`, or `-diff`, which it implies.
    Binary(&'static str),
}

impl DiffAttributes<'_> {
    /// `name=value` as `.gitattributes` assigns it to `path`: `filter=lfs`, `diff=word`.
    pub fn value(&mut self, path: &str, name: &str) -> Option<String> {
        let stack = self.stack.as_mut()?;
        let mut outcome = stack.selected_attribute_matches([name]);
        stack
            .at_entry(path, None)
            .ok()?
            .matching_attributes(&mut outcome);
        outcome
            .iter_selected()
            .find(|found| found.assignment.name.as_str() == name)
            .and_then(|found| match found.assignment.state {
                gix::attrs::StateRef::Value(value) => Some(value.as_bstr().to_string()),
                _ => None,
            })
    }

    pub fn content(&mut self, path: &str) -> DiffContent {
        use gix::attrs::StateRef;
        let Some(stack) = self.stack.as_mut() else {
            return DiffContent::Detect;
        };
        let mut outcome = stack.selected_attribute_matches(["binary", "diff", "text"]);
        match stack.at_entry(path, None) {
            Ok(platform) => platform.matching_attributes(&mut outcome),
            Err(err) => {
                tracing::error!(error = ?err, path, context = "reading .gitattributes for a diff");
                return DiffContent::Detect;
            }
        };
        let state = |name: &str| {
            outcome
                .iter_selected()
                .find(|found| found.assignment.name.as_str() == name)
                .map(|found| found.assignment.state)
        };
        if matches!(state("binary"), Some(StateRef::Set)) {
            return DiffContent::Binary("binary");
        }
        match (state("diff"), state("text")) {
            (Some(StateRef::Unset), _) => DiffContent::Binary("-diff"),
            (Some(StateRef::Set), _) | (_, Some(StateRef::Set)) => DiffContent::Text,
            _ => DiffContent::Detect,
        }
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
            .current_index()
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
        if recorded.is_none() && previous.is_none() {
            return Ok(None);
        }

        Ok(Some(crate::SubmodulePointer {
            checked_out: self.root().join(path).join(".git").exists()
                || self.root().join(path).join("HEAD").exists(),
            in_index: self.gitlink_in_index(path).is_some(),
            previous: previous.filter(|before| recorded.as_ref() != Some(before)),
            recorded,
        }))
    }

    fn gitlink_in_index(&self, path: &str) -> Option<String> {
        let index = self.current_index().ok()?;
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
        let snapshot = self.diff_snapshot(spec)?;
        let source = snapshot.rename_source(path)?;
        snapshot.sides(source.unwrap_or(path), path)
    }

    /// The sides with the old one read at `old_path`: the source of a rename or copy.
    pub fn diff_sides_from(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> Result<DiffSides> {
        self.diff_snapshot(spec)?.sides(old_path, path)
    }

    /// What `diff_sides_from` would read, by size alone: a side too large to show is never read.
    pub fn side_sizes_from(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> Result<(Option<u64>, Option<u64>)> {
        self.diff_snapshot(spec)?.sizes(old_path, path)
    }

    /// The object id git gives each side `diff_sides_from` would read: a tree's or the
    /// index's own, and for the working file the one `git hash-object` prints, through the
    /// clean filters. `None` where the file is absent, and for a working file past
    /// `MAX_HASHED_BYTES`, which is not read for it.
    pub fn side_ids_from(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> Result<(Option<String>, Option<String>)> {
        self.diff_snapshot(spec)?.ids(old_path, path)
    }

    /// Where the old side keeps a file it has no `path` for: the source of the rename or
    /// copy the file list shows as `← old.txt`. Looked for only then, since finding it
    /// costs a tree diff with rename tracking.
    pub fn rename_source(&self, spec: &DiffSpec, path: &str) -> Result<Option<String>> {
        Ok(self
            .diff_snapshot(spec)?
            .rename_source(path)?
            .map(str::to_owned))
    }

    /// What `diff_sides` would read, by size alone.
    pub fn side_sizes(&self, spec: &DiffSpec, path: &str) -> Result<(Option<u64>, Option<u64>)> {
        let snapshot = self.diff_snapshot(spec)?;
        let source = snapshot.rename_source(path)?;
        snapshot.sizes(source.unwrap_or(path), path)
    }

    /// The spec resolved once for any number of paths: its trees, one read of the index,
    /// and the renames, found by one tree diff when a path first needs them.
    pub fn diff_snapshot(&self, spec: &DiffSpec) -> Result<DiffSnapshot<'_>> {
        let (old, new, renames) = match spec {
            DiffSpec::CommitVsParent { oid } => {
                let commit = self.commit_at(oid)?;
                let new = Side::Tree(tree_of(commit.tree(), oid)?);
                match commit.parent_ids().next() {
                    Some(parent) => {
                        let commit = self.repo.find_commit(parent.detach()).map_err(|err| {
                            GitError::InvalidState(format!("{parent} is not a commit: {err}"))
                        })?;
                        (
                            Side::Tree(tree_of(commit.tree(), parent)?),
                            new,
                            Renames::Trees,
                        )
                    }
                    None => (Side::Absent, new, Renames::None),
                }
            }
            DiffSpec::CommitVsCommit { a, b } => (
                Side::Tree(self.tree_at(a)?),
                Side::Tree(self.tree_at(b)?),
                Renames::Trees,
            ),
            DiffSpec::WorkTreeVsIndex => (Side::Index, Side::WorkTree, Renames::None),
            DiffSpec::IndexVsHead => {
                let old = match self.head()? {
                    crate::Head::Unborn { .. } => Side::Absent,
                    _ => Side::Tree(self.tree_at("HEAD")?),
                };
                (old, Side::Index, Renames::Staged)
            }
            DiffSpec::CommitVsWorkTree { oid } => (
                Side::Tree(self.tree_at(oid)?),
                Side::WorkTree,
                Renames::None,
            ),
        };
        let index = match (&old, &new) {
            (Side::Tree(_) | Side::Absent, Side::Tree(_)) => None,
            _ => Some(self.current_index()?),
        };
        Ok(DiffSnapshot {
            handle: self,
            old,
            new,
            renames,
            index,
            sources: std::cell::OnceCell::new(),
        })
    }

    fn commit_at(&self, rev: &str) -> Result<gix::Commit<'_>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        self.repo
            .find_commit(id.detach())
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))
    }

    fn tree_at(&self, rev: &str) -> Result<gix::Tree<'_>> {
        tree_of(self.commit_at(rev)?.tree(), rev)
    }

    /// Every rename and copy between two trees, new path to old; the first found wins.
    fn renames_between(
        &self,
        before: &gix::Tree<'_>,
        after: &gix::Tree<'_>,
    ) -> Result<HashMap<String, String>> {
        let files = self.files_between_trees(before, after, crate::DEFAULT_SIMILARITY)?;
        let mut sources = HashMap::new();
        for file in files {
            if let Some(old_path) = file.old_path {
                sources.entry(file.path).or_insert(old_path);
            }
        }
        Ok(sources)
    }

    /// The same rename tracking the Staged list gets from status; the last found wins.
    fn staged_renames(
        &self,
        head: &gix::Tree<'_>,
        index: &gix::index::State,
    ) -> Result<HashMap<String, String>> {
        let mut sources = HashMap::new();
        self.repo
            .tree_index_status(
                &head.id,
                index,
                None,
                gix::status::tree_index::TrackRenames::AsConfigured,
                |change, _, _| {
                    if let gix::diff::index::ChangeRef::Rewrite {
                        source_location,
                        location,
                        ..
                    } = change
                    {
                        sources.insert(location.to_string(), source_location.to_string());
                    }
                    Ok::<_, gix::Exn>(std::ops::ControlFlow::Continue(()))
                },
            )
            .map_err(|err| {
                GitError::Internal(format!("cannot compare the index with HEAD: {err}"))
            })?;
        Ok(sources)
    }

    fn id_in_index(index: &gix::index::State, path: &str) -> Option<String> {
        index
            .entry_by_path(path.into())
            .filter(|entry| !entry.mode.is_submodule())
            .map(|entry| entry.id.to_string())
    }

    fn id_on_disk(&self, path: &str) -> Result<Option<String>> {
        if !self
            .size_on_disk(path)?
            .is_some_and(|size| size <= MAX_HASHED_BYTES)
        {
            return Ok(None);
        }
        let Some(bytes) = self.blob_on_disk(path)? else {
            return Ok(None);
        };
        let bytes = if is_symlink(&self.root().join(path)) {
            bytes
        } else {
            self.cleaned(path, bytes)?
        };
        gix::objs::compute_hash(self.repo.object_hash(), gix::objs::Kind::Blob, &bytes)
            .map(|id| Some(id.to_string()))
            .map_err(|err| GitError::Internal(format!("cannot hash {path}: {err}")))
    }

    /// An entry marked skip-worktree (sparse checkout) or assume-unchanged stands for the
    /// working file, whatever is on disk or is not: read from disk, a missing one looked
    /// deleted, and staging that deleted it.
    fn index_stands_in(index: &gix::index::State, path: &str) -> bool {
        use gix::index::entry::Flags;
        index.entry_by_path(path.into()).is_some_and(|entry| {
            entry
                .flags
                .intersects(Flags::SKIP_WORKTREE | Flags::ASSUME_VALID)
        })
    }

    fn blob_id_in(tree: &gix::Tree<'_>, path: &str) -> Result<Option<gix::ObjectId>> {
        let Some(entry) = tree
            .lookup_entry_by_path(path)
            .map_err(|err| GitError::Internal(format!("cannot look up {path}: {err}")))?
        else {
            return Ok(None);
        };
        Ok(entry.mode().is_blob_or_symlink().then(|| entry.object_id()))
    }

    fn size_in_index(&self, index: &gix::index::State, path: &str) -> Result<Option<u64>> {
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
        self.blob_in_state(state(&self.current_index()?), path)
    }

    fn blob_in_state(&self, index: &gix::index::State, path: &str) -> Result<Option<Vec<u8>>> {
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
        Ok(Some(object.into_blob().take_data()))
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
        if !entry.mode().is_blob_or_symlink() {
            return Ok(None);
        }

        let object = entry
            .object()
            .map_err(|err| GitError::Internal(format!("cannot read {path}: {err}")))?;
        Ok(Some(object.into_blob().take_data()))
    }
}

pub(crate) fn is_symlink(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// A [`DiffSpec`] resolved for a batch of paths (`RepoHandle::diff_snapshot`).
pub struct DiffSnapshot<'repo> {
    handle: &'repo RepoHandle,
    old: Side<'repo>,
    new: Side<'repo>,
    renames: Renames,
    index: Option<gix::worktree::IndexPersistedOrInMemory>,
    /// New path to old, for every rename and copy; found the first time a path needs it.
    sources: std::cell::OnceCell<HashMap<String, String>>,
}

impl std::fmt::Debug for DiffSnapshot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffSnapshot")
            .field("renames", &self.renames)
            .finish_non_exhaustive()
    }
}

enum Side<'repo> {
    Tree(gix::Tree<'repo>),
    Absent,
    Index,
    WorkTree,
}

/// Where the old side of a renamed file is looked for; only these read it at the old path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Renames {
    None,
    Trees,
    Staged,
}

impl<'repo> DiffSnapshot<'repo> {
    /// `RepoHandle::rename_source` for this spec.
    pub fn rename_source(&self, path: &str) -> Result<Option<&str>> {
        let Side::Tree(old) = &self.old else {
            return Ok(None);
        };
        if self.renames == Renames::None || !matches!(old.lookup_entry_by_path(path), Ok(None)) {
            return Ok(None);
        }
        if self.sources.get().is_none() {
            let found = match (&self.new, &self.index) {
                (Side::Tree(new), _) => self.handle.renames_between(old, new)?,
                (_, Some(index)) => self.handle.staged_renames(old, index)?,
                _ => HashMap::new(),
            };
            // Only `None` was seen above, and nothing else fills it.
            let _ = self.sources.set(found);
        }
        Ok(self
            .sources
            .get()
            .and_then(|sources| sources.get(path))
            .map(String::as_str))
    }

    /// `RepoHandle::diff_sides_from` for this spec.
    pub fn sides(&self, old_path: &str, path: &str) -> Result<DiffSides> {
        Ok((
            self.read(&self.old, self.old_at(old_path, path))?,
            self.read(&self.new, path)?,
        ))
    }

    /// `RepoHandle::side_sizes_from` for this spec.
    pub fn sizes(&self, old_path: &str, path: &str) -> Result<(Option<u64>, Option<u64>)> {
        Ok((
            self.size(&self.old, self.old_at(old_path, path))?,
            self.size(&self.new, path)?,
        ))
    }

    /// `RepoHandle::side_ids_from` for this spec.
    pub fn ids(&self, old_path: &str, path: &str) -> Result<(Option<String>, Option<String>)> {
        Ok((
            self.id(&self.old, self.old_at(old_path, path))?,
            self.id(&self.new, path)?,
        ))
    }

    fn old_at<'p>(&self, old_path: &'p str, path: &'p str) -> &'p str {
        if self.renames == Renames::None {
            path
        } else {
            old_path
        }
    }

    /// Where a side keeps `path`: the index stands for the working file where an entry
    /// is marked skip-worktree (sparse checkout) or assume-unchanged.
    fn source<'s>(&'s self, side: &'s Side<'repo>, path: &str) -> Source<'s, 'repo> {
        let index = self.index.as_ref().map(state);
        match (side, index) {
            (Side::Tree(tree), _) => Source::Tree(tree),
            (Side::Index, Some(index)) => Source::Index(index),
            (Side::WorkTree, Some(index)) if RepoHandle::index_stands_in(index, path) => {
                Source::Index(index)
            }
            (Side::WorkTree, _) => Source::Disk,
            (Side::Absent | Side::Index, _) => Source::Absent,
        }
    }

    fn read(&self, side: &Side<'_>, path: &str) -> Result<Option<Vec<u8>>> {
        match self.source(side, path) {
            Source::Tree(tree) => self.handle.blob_in(tree, path),
            Source::Index(index) => self.handle.blob_in_state(index, path),
            Source::Disk => self.handle.blob_on_disk(path),
            Source::Absent => Ok(None),
        }
    }

    fn size(&self, side: &Side<'_>, path: &str) -> Result<Option<u64>> {
        match self.source(side, path) {
            Source::Tree(tree) => RepoHandle::blob_id_in(tree, path)?
                .map(|id| self.handle.blob_size(id, path))
                .transpose(),
            Source::Index(index) => self.handle.size_in_index(index, path),
            Source::Disk => self.handle.size_on_disk(path),
            Source::Absent => Ok(None),
        }
    }

    fn id(&self, side: &Side<'_>, path: &str) -> Result<Option<String>> {
        match self.source(side, path) {
            Source::Tree(tree) => Ok(RepoHandle::blob_id_in(tree, path)?.map(|id| id.to_string())),
            Source::Index(index) => Ok(RepoHandle::id_in_index(index, path)),
            Source::Disk => self.handle.id_on_disk(path),
            Source::Absent => Ok(None),
        }
    }
}

enum Source<'s, 'repo> {
    Tree(&'s gix::Tree<'repo>),
    Index(&'s gix::index::State),
    Disk,
    Absent,
}

fn state(index: &gix::worktree::IndexPersistedOrInMemory) -> &gix::index::State {
    index
}

fn tree_of<'repo>(
    tree: std::result::Result<gix::Tree<'repo>, impl std::fmt::Display>,
    rev: impl std::fmt::Display,
) -> Result<gix::Tree<'repo>> {
    tree.map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))
}
