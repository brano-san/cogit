use crate::{FileStatus, GitError, Head, RepoHandle, Result};
use std::collections::HashSet;

impl RepoHandle {
    pub fn stage(&self, paths: &[String]) -> Result<()> {
        self.run_paths(add_command(paths.len()), paths)
    }

    /// What git sees as changed, the whole tree: without a path list git matches nothing
    /// against its entries, which on two thousand files is a tenth of the time (R-311).
    /// `files` is how many the list showed; it only picks how the blobs are written.
    pub fn stage_all(&self, files: usize) -> Result<()> {
        self.run_git(add_command(files)).map(drop)
    }

    pub fn unstage(&self, paths: &[String]) -> Result<()> {
        require_paths(paths)?;
        // `restore --staged` resolves HEAD, which does not exist before the first commit.
        // `-f` passes a file edited since `add`; with `--cached` the disk is never touched.
        if matches!(self.head()?, Head::Unborn { .. }) {
            return self.run_paths(&["rm", "--cached", "-r", "-f"], paths);
        }
        self.run_paths(&["restore", "--staged"], &self.with_rename_sources(paths)?)
    }

    /// `paths` plus the old name of each staged rename among them. The Staged list shows a
    /// rename as one row under its new name, and acting on that name alone left the
    /// deletion of the old one staged.
    pub(crate) fn with_rename_sources(&self, paths: &[String]) -> Result<Vec<String>> {
        let Ok(tree) = self.repo.head_tree() else {
            return Ok(paths.to_vec());
        };
        // A rename's new name is not in HEAD; when every path is, there is nothing to diff.
        let added: HashSet<&str> = paths
            .iter()
            .map(String::as_str)
            .filter(|path| matches!(tree.lookup_entry_by_path(path), Ok(None)))
            .collect();
        if added.is_empty() {
            return Ok(paths.to_vec());
        }
        let index = self
            .repo
            .open_index()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let mut all = paths.to_vec();
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
                        copy: false,
                        ..
                    } = change
                        && added.contains(location.to_string().as_str())
                    {
                        all.push(source_location.to_string());
                    }
                    Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
                },
            )
            .map_err(|err| {
                GitError::Internal(format!("cannot compare the index with HEAD: {err}"))
            })?;
        Ok(all)
    }

    /// A tracked file comes back from the index; an untracked one is removed outright.
    pub fn discard(&self, paths: &[String]) -> Result<()> {
        require_paths(paths)?;

        let untracked: HashSet<String> = self
            .worktree_files()?
            .unstaged
            .into_iter()
            .filter(|entry| entry.status == FileStatus::Untracked)
            .map(|entry| entry.path)
            .collect();

        let (to_clean, to_restore): (Vec<String>, Vec<String>) = paths
            .iter()
            .cloned()
            .partition(|path| untracked.contains(path));

        if !to_restore.is_empty() {
            self.run_paths(&["restore", "--worktree"], &to_restore)?;
        }
        for batch in crate::runner::command_line_batches(&to_clean) {
            self.run_paths(&["clean", "-fd"], batch)?;
        }
        Ok(())
    }

    fn run_paths(&self, prefix: &[&str], paths: &[String]) -> Result<()> {
        require_paths(paths)?;
        self.run_git_paths(prefix, paths).map(drop)
    }
}

/// From here on the blobs go into one pack rather than a loose file each: git streams a
/// blob larger than `core.bigFileThreshold` straight into a pack, one per command, unless
/// it converts the content on the way in. The bar keeps packs as rare as loose objects make
/// `gc --auto`: 50 packs (`gc.autoPackLimit`) against 6 700 objects (`gc.auto`) (R-312).
const PACKED_FROM: usize = 200;

fn add_command(files: usize) -> &'static [&'static str] {
    if files >= PACKED_FROM {
        &["-c", "core.bigFileThreshold=1", "add", "--all"]
    } else {
        &["add", "--all"]
    }
}

/// A path list for a command that would act on everything when given none.
pub(crate) fn require_paths(paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Err(GitError::InvalidState(
            "no paths given; refusing to act on the whole repository".to_owned(),
        ));
    }
    Ok(())
}

impl RepoHandle {
    /// Only the executable bit; `git add` would stage the content change with it.
    pub fn stage_mode(&self, path: &str, executable: bool) -> Result<()> {
        // `--chmod` re-reads the file, so re-register with the blob already indexed.
        let oid = match self.index_blob(path)? {
            IndexBlob::Staged(oid) => oid,
            IndexBlob::Conflicted => {
                return Err(GitError::InvalidState(format!(
                    "{path} is conflicted; resolve it first"
                )));
            }
            IndexBlob::Absent => {
                return Err(GitError::InvalidState(format!("{path} is not tracked")));
            }
        };

        let mode = if executable { "100755" } else { "100644" };
        let entry = format!("{mode},{oid},{path}");
        self.run_git(&["update-index", "--cacheinfo", &entry])
            .map(drop)
    }
}

pub(crate) enum IndexBlob {
    Staged(String),
    Conflicted,
    Absent,
}

impl RepoHandle {
    /// What the index holds at exactly `path`, read through `gix`. Asked of `ls-files`,
    /// "not tracked" was a failed command in the journal, and a notification with it.
    pub(crate) fn index_blob(&self, path: &str) -> Result<IndexBlob> {
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let backing = index.path_backing();
        let wanted = gix::bstr::BStr::new(path.as_bytes());
        let mut conflicted = false;
        for entry in index.entries() {
            if entry.path_in(backing) != wanted {
                continue;
            }
            if entry.stage() as u8 == 0 {
                return Ok(IndexBlob::Staged(entry.id.to_string()));
            }
            conflicted = true;
        }
        Ok(if conflicted {
            IndexBlob::Conflicted
        } else {
            IndexBlob::Absent
        })
    }

    /// Whether the index has `path`, or anything under it when it is a folder.
    pub(crate) fn tracks(&self, path: &str) -> Result<bool> {
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let backing = index.path_backing();
        let folder = format!("{path}/");
        Ok(index.entries().iter().any(|entry| {
            let name = entry.path_in(backing);
            name == gix::bstr::BStr::new(path.as_bytes()) || name.starts_with(folder.as_bytes())
        }))
    }
}
