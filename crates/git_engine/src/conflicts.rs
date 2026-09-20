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
}

impl ConflictSides {
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
        }
    }
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

    pub fn resolve_with(&self, path: &str, side: ConflictSide) -> Result<()> {
        let content = self
            .stage_blob(path, side)
            .ok_or_else(|| GitError::InvalidState(format!("{path} has no {side:?} side")))?;
        self.write_resolution(path, &content)
    }

    pub fn resolve_with_text(&self, path: &str, text: &str) -> Result<()> {
        if !self.conflicted_paths()?.iter().any(|p| p == path) {
            return Err(GitError::InvalidState(format!("{path} is not conflicted")));
        }
        self.write_resolution(path, text.as_bytes())
    }

    /// Write and stage in one step, or `git merge --continue` refuses a file that looks done.
    fn write_resolution(&self, path: &str, content: &[u8]) -> Result<()> {
        std::fs::write(self.root().join(path), content)?;
        self.run_git(&["add", "--", path]).map(drop)
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
