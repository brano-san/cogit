use crate::repo::env_free;
use crate::{GitError, RepoHandle, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A repository opened once — discovery, config files, ref store — and handed out as a
/// fresh [`RepoHandle`] per call; opening it anew cost ~0.8 ms a call.
///
/// What the snapshot does not follow by itself is the config, so every file it was read
/// from (and the repository's own, even if absent) is stamped; [`SharedRepo::is_current`]
/// says whether one changed. Refs, HEAD and new objects are read from disk as they are
/// asked for; the index through [`RepoHandle::current_index`]. Nothing stays mapped between
/// calls: Windows refuses to replace or delete a mapped file, and `git` rewrites
/// `packed-refs` and deletes packs.
pub struct SharedRepo {
    template: gix::ThreadSafeRepository,
    stamps: Vec<(PathBuf, Option<Stamp>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: SystemTime,
    len: u64,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: meta.modified().ok()?,
        len: meta.len(),
    })
}

impl std::fmt::Debug for SharedRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRepo")
            .field("git_dir", &self.template.git_dir())
            .finish_non_exhaustive()
    }
}

impl SharedRepo {
    /// Opens the repository whose root is `root`, as [`RepoHandle::open_root`] does.
    pub fn open(root: &Path) -> Result<Self> {
        let options = env_free();
        let mut template = gix::ThreadSafeRepository::discover_opts(
            root,
            gix::discover::upwards::Options::default(),
            gix::sec::trust::Mapping {
                full: options.clone(),
                reduced: options,
            },
        )
        .map_err(|err| GitError::RepoNotFound(format!("{}: {err}", root.display())))?;
        crate::repo::rooted_at(
            template.work_dir().unwrap_or_else(|| template.git_dir()),
            root,
        )?;
        template.refs.set_packed_buffer_mmap_threshold(u64::MAX);
        let stamps = config_sources(&template.to_thread_local())
            .into_iter()
            .map(|path| {
                let now = stamp(&path);
                (path, now)
            })
            .collect();
        Ok(Self { template, stamps })
    }

    /// False once a config file it read has changed, appeared or gone — or the repository
    /// itself has, since its `config` goes with it.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.stamps.iter().all(|(path, was)| stamp(path) == *was)
    }

    /// A handle with an object store of its own, so the packs it maps are let go with it.
    pub fn handle(&self) -> Result<RepoHandle> {
        let mut sync = self.template.clone();
        let store = gix::odb::Store::try_from(&*self.template.objects)
            .map_err(|err| GitError::Internal(format!("cannot open the object store: {err}")))?;
        sync.objects = gix::features::threading::OwnShared::new(store);
        Ok(RepoHandle::from_repo(sync.to_thread_local()))
    }
}

impl RepoHandle {
    /// The index as it is on disk now. gix rereads the snapshot every handle of a
    /// `SharedRepo` shares only on a strictly newer mtime, so two writes within one tick of
    /// the file system's clock left it at the first (R-481). The checksum git writes at the
    /// end of the file tells; `index.skipHash` writes zeros, and then it is read anew.
    pub(crate) fn current_index(&self) -> Result<gix::worktree::IndexPersistedOrInMemory> {
        let snapshot = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let current = match snapshot.checksum() {
            None => true,
            Some(sum) if sum.is_null() => false,
            Some(sum) => trailer(self.repo.index_path(), sum.as_slice().len())
                .is_some_and(|on_disk| on_disk == sum.as_slice()),
        };
        if current {
            return Ok(snapshot.into());
        }
        match self.repo.open_index() {
            Ok(fresh) => Ok(fresh.into()),
            Err(err) => {
                tracing::error!(error = ?err, context = "rereading the index, the shared snapshot is used");
                Ok(snapshot.into())
            }
        }
    }

    /// `repo.status()` against [`RepoHandle::current_index`].
    pub(crate) fn status_platform(
        &self,
    ) -> Result<gix::status::Platform<'_, gix::progress::Discard>> {
        Ok(self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .index(self.current_index()?))
    }
}

fn trailer(path: PathBuf, len: usize) -> Option<Vec<u8>> {
    use std::io::{Read as _, Seek as _};
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(std::io::SeekFrom::End(-i64::try_from(len).ok()?))
        .ok()?;
    let mut bytes = vec![0; len];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

/// Every file the config came from, plus the repository's own two in case they appear.
fn config_sources(repo: &gix::Repository) -> Vec<PathBuf> {
    let snapshot = repo.config_snapshot();
    let mut paths: Vec<PathBuf> = snapshot
        .plumbing()
        .sections()
        .filter_map(|section| section.meta().path.clone())
        .collect();
    paths.push(repo.common_dir().join("config"));
    paths.push(repo.git_dir().join("config.worktree"));
    // `includeIf "onbranch:…"` is decided by HEAD at open.
    paths.push(repo.git_dir().join("HEAD"));
    paths.sort();
    paths.dedup();
    paths
}
