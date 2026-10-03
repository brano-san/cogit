use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictSide {
    Base,
    Ours,
    Theirs,
}

/// Any side can be missing: a file added on one side has no base.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSides {
    pub base: Option<Vec<u8>>,
    pub ours: Option<Vec<u8>>,
    pub theirs: Option<Vec<u8>>,
    #[serde(skip)]
    pub kind: EntryKind,
}

/// What the sides are: a link or a submodule is taken whole, whatever its bytes look like.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EntryKind {
    #[default]
    Regular,
    Symlink,
    Submodule,
}

/// Named rather than a tuple: positional optional strings reorder silently across IPC.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictText {
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    /// A side is binary or not UTF-8: it is taken whole, never merged or edited as text.
    pub binary: bool,
}

impl ConflictSides {
    /// Merging and editing work on text; a byte they cannot hold would be written back as
    /// a replacement character and staged.
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.kind == EntryKind::Regular
            && [&self.base, &self.ours, &self.theirs]
                .into_iter()
                .flatten()
                .all(|side| is_text(side))
    }

    /// Lossy on purpose: a conflict the user cannot see is worse than one rendered oddly.
    pub fn to_text(&self) -> ConflictText {
        let text = |side: &Option<Vec<u8>>| {
            side.as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        };
        ConflictText {
            base: text(&self.base),
            ours: text(&self.ours),
            theirs: text(&self.theirs),
            binary: !self.is_text(),
        }
    }
}

/// The merge view joins its lines with LF and ends on a newline. The file keeps what our
/// side has instead: its dominant line ending, and no final newline where it had none.
fn shaped_like(text: &str, like: &[u8]) -> String {
    let crlf = like.windows(2).filter(|pair| *pair == b"\r\n").count();
    let lf = like.iter().filter(|&&byte| byte == b'\n').count() - crlf;
    let mut out = text.replace("\r\n", "\n");
    if !like.is_empty() && !like.ends_with(b"\n") && out.ends_with('\n') {
        out.pop();
    }
    if crlf > lf {
        out = out.replace('\n', "\r\n");
    }
    out
}

/// Git's binary rule — a NUL in the first 8000 bytes — and valid UTF-8.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.iter().take(8000).any(|&byte| byte == 0) && std::str::from_utf8(bytes).is_ok()
}

impl ConflictSide {
    fn stage(self) -> u8 {
        match self {
            Self::Base => 1,
            Self::Ours => 2,
            Self::Theirs => 3,
        }
    }
}

impl RepoHandle {
    pub fn conflicted_paths(&self) -> Result<Vec<String>> {
        let mut paths: Vec<String> = self
            .worktree_files()?
            .unstaged
            .into_iter()
            .filter(|entry| entry.status == crate::FileStatus::Conflicted)
            .map(|entry| entry.path)
            .collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn conflict_sides(&self, path: &str) -> Result<ConflictSides> {
        if !self.conflicted_paths()?.iter().any(|p| p == path) {
            return Err(GitError::InvalidState(format!("{path} is not conflicted")));
        }
        let mut sides = ConflictSides::default();
        for (slot, side) in [
            (&mut sides.base, ConflictSide::Base),
            (&mut sides.ours, ConflictSide::Ours),
            (&mut sides.theirs, ConflictSide::Theirs),
        ] {
            if let Some((bytes, kind)) = self.stage_entry(path, side)? {
                *slot = Some(bytes);
                if kind != EntryKind::Regular {
                    sides.kind = kind;
                }
            }
        }
        Ok(sides)
    }

    /// Checked out by git, not written from the blob: the eol and smudge filters (LFS)
    /// apply to a stage as they do to any checkout. Ours or theirs missing is the side that
    /// deleted the file, and taking it deletes the file.
    pub fn resolve_with(&self, path: &str, side: ConflictSide) -> Result<()> {
        let entry = self.stage_entry(path, side)?;
        if entry.is_none() {
            if side == ConflictSide::Base || !self.conflicted_paths()?.iter().any(|p| p == path) {
                return Err(GitError::InvalidState(format!(
                    "{path} has no {side:?} side"
                )));
            }
            return self.run_git_literal(&["rm", "-q", "--", path]).map(drop);
        }
        if let Some((oid, EntryKind::Submodule)) = entry {
            // `add` would stage the submodule's current HEAD, not the side's commit.
            let oid = String::from_utf8_lossy(&oid);
            return self
                .run_git_literal(&[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("160000,{oid},{path}"),
                ])
                .map(drop);
        }
        let stage = format!("--stage={}", side.stage());
        self.run_git_literal(&["checkout-index", "-f", &stage, "--", path])?;
        self.run_git_literal(&["add", "--", path]).map(drop)
    }

    /// Written and staged in one step, or `git merge --continue` refuses a file that looks
    /// done; then checked out again, so the eol and smudge filters apply as to any file.
    pub fn resolve_with_text(&self, path: &str, text: &str) -> Result<()> {
        let sides = self.conflict_sides(path)?;
        if sides.kind != EntryKind::Regular {
            return Err(GitError::InvalidState(format!(
                "{path} is a symbolic link or a submodule: take one side whole"
            )));
        }
        if !sides.is_text() {
            return Err(GitError::InvalidState(format!(
                "{path} is binary or not UTF-8: take one side whole"
            )));
        }
        let like = [&sides.ours, &sides.theirs, &sides.base]
            .into_iter()
            .find_map(Option::as_deref)
            .unwrap_or_default();
        let file = self.root().join(path);
        if crate::blobs::is_symlink(&file) {
            return Err(GitError::InvalidState(format!(
                "{path} is a symbolic link: take one side whole"
            )));
        }
        std::fs::write(&file, shaped_like(text, like))?;
        self.run_git_literal(&["add", "--", path])?;
        // `checkout-index` passes over a file that matches the index, however it is written.
        std::fs::remove_file(&file)?;
        self.run_git_literal(&["checkout-index", "--", path])
            .map(drop)
    }

    /// The working file as it is, kept in the object store before a resolution writes over
    /// it: hand edits made in an editor are in it (INV-12). `None` when there is no file.
    pub fn keep_worktree_file(&self, path: &str) -> Result<Option<String>> {
        let file = self.root().join(path);
        if crate::blobs::is_symlink(&file) || !file.is_file() {
            return Ok(None);
        }
        let kept = self.run_git_literal(&["hash-object", "-w", "--no-filters", "--", path])?;
        Ok(Some(kept.stdout.trim().to_owned()))
    }

    /// A resolution undone: the conflict again where git still has its sides (resolve-undo),
    /// then the working file as it was kept.
    pub fn unresolve(&self, path: &str, kept: Option<&str>) -> Result<()> {
        let conflict = self.run_git_literal(&["checkout", "-m", "--", path]);
        let Some(kept) = kept else {
            return conflict.map(drop);
        };
        if let Err(err) = conflict {
            // A deletion taken leaves too little to conflict again ("does not have all
            // necessary versions"); the file itself still comes back.
            tracing::warn!(error = ?err, path, "the conflict cannot be recreated");
        }
        let id = gix::ObjectId::from_hex(kept.as_bytes())
            .map_err(|err| GitError::Internal(format!("bad kept object {kept}: {err}")))?;
        let bytes = self
            .repo
            .find_object(id)
            .map_err(|err| GitError::Internal(format!("cannot read the kept {path}: {err}")))?
            .detach()
            .data;
        let file = self.root().join(path);
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        if crate::blobs::is_symlink(&file) {
            std::fs::remove_file(&file)?;
        }
        std::fs::write(file, bytes)?;
        Ok(())
    }

    /// `None` only when the stage has no entry. A gitlink's content is its commit's hex id:
    /// that commit is not in this repository's object store.
    fn stage_entry(&self, path: &str, side: ConflictSide) -> Result<Option<(Vec<u8>, EntryKind)>> {
        let index = self.current_index()?;
        let backing = index.path_backing();
        let Some(entry) = index.entries().iter().find(|entry| {
            entry.stage() as u8 == side.stage()
                && entry.path_in(backing) == gix::bstr::BStr::new(path.as_bytes())
        }) else {
            return Ok(None);
        };
        if entry.mode.is_submodule() {
            return Ok(Some((
                entry.id.to_string().into_bytes(),
                EntryKind::Submodule,
            )));
        }
        let object = self
            .repo
            .find_object(entry.id)
            .map_err(|err| GitError::Internal(format!("cannot read {path}: {err}")))?;
        let kind = if entry.mode == gix::index::entry::Mode::SYMLINK {
            EntryKind::Symlink
        } else {
            EntryKind::Regular
        };
        Ok(Some((object.into_blob().data.clone(), kind)))
    }
}
