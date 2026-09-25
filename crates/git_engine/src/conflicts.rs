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
        [&self.base, &self.ours, &self.theirs]
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
        Ok(ConflictSides {
            base: self.stage_blob(path, ConflictSide::Base),
            ours: self.stage_blob(path, ConflictSide::Ours),
            theirs: self.stage_blob(path, ConflictSide::Theirs),
        })
    }

    /// Checked out by git, not written from the blob: the eol and smudge filters (LFS)
    /// apply to a stage as they do to any checkout. Ours or theirs missing is the side that
    /// deleted the file, and taking it deletes the file.
    pub fn resolve_with(&self, path: &str, side: ConflictSide) -> Result<()> {
        if self.stage_blob(path, side).is_none() {
            if side == ConflictSide::Base || !self.conflicted_paths()?.iter().any(|p| p == path) {
                return Err(GitError::InvalidState(format!(
                    "{path} has no {side:?} side"
                )));
            }
            return self.run_git_literal(&["rm", "-q", "--", path]).map(drop);
        }
        let stage = format!("--stage={}", side.stage());
        self.run_git_literal(&["checkout-index", "-f", &stage, "--", path])?;
        self.run_git_literal(&["add", "--", path]).map(drop)
    }

    /// Written and staged in one step, or `git merge --continue` refuses a file that looks
    /// done; then checked out again, so the eol and smudge filters apply as to any file.
    pub fn resolve_with_text(&self, path: &str, text: &str) -> Result<()> {
        let sides = self.conflict_sides(path)?;
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
        std::fs::write(&file, shaped_like(text, like))?;
        self.run_git_literal(&["add", "--", path])?;
        // `checkout-index` passes over a file that matches the index, however it is written.
        std::fs::remove_file(&file)?;
        self.run_git_literal(&["checkout-index", "--", path])
            .map(drop)
    }

    fn stage_blob(&self, path: &str, side: ConflictSide) -> Option<Vec<u8>> {
        let index = self.repo.index_or_empty().ok()?;
        let backing = index.path_backing();
        let entry = index.entries().iter().find(|entry| {
            entry.stage() as u8 == side.stage()
                && entry.path_in(backing) == gix::bstr::BStr::new(path.as_bytes())
        })?;
        let object = self.repo.find_object(entry.id).ok()?;
        Some(object.into_blob().data.clone())
    }
}
