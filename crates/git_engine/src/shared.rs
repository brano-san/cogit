use crate::repo::env_free;
use crate::{GitError, RepoHandle, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A repository opened once — discovery, config files, ref store — and handed out as a
/// fresh [`RepoHandle`] per call; opening it anew cost ~0.8 ms a call.
///
/// What the snapshot does not follow by itself is the config, so every file it was read
/// from (and the repository's own, even if absent) is stamped; [`SharedRepo::is_current`]
/// says whether one changed. Refs, HEAD, the index and new objects are read from disk as
/// they are asked for. Nothing stays mapped between calls: Windows refuses to replace or
/// delete a mapped file, and `git` rewrites `packed-refs` and deletes packs.
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
