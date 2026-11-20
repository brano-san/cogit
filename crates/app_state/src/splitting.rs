use crate::{AppState, RepoId, short};
use git_engine::{FileMode, FileStatus, GitError};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitFile {
    pub path: String,
    pub blocks: Vec<String>,
    pub whole: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SplitPreview {
    pub message: String,
    pub files: Vec<SplitFile>,
}

struct SplitFileDiff {
    file: SplitFile,
    rename_source: Option<String>,
    hunks: Vec<diff_engine::Hunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitBlock {
    pub path: String,
    pub hunk: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitPart {
    pub message: String,
    pub blocks: Vec<SplitBlock>,
}

fn options() -> diff_engine::DiffOptions {
    diff_engine::DiffOptions {
        word_diff: false,
        detect_moves: false,
        ..diff_engine::DiffOptions::default()
    }
}

fn note_of(diff: &diff_engine::FileDiff) -> Option<&'static str> {
    use diff_engine::FileDiff;
    match diff {
        FileDiff::Text {
            converted: Some(_), ..
        } => Some("converted text"),
        FileDiff::Text {
            lossy_encoding: true,
            ..
        } => Some("not UTF-8"),
        FileDiff::Text { .. } | FileDiff::Unchanged => None,
        FileDiff::EolOnly { .. } => Some("line endings only"),
        FileDiff::Binary { .. } => Some("binary"),
        FileDiff::Image { .. } => Some("image"),
        FileDiff::TooLarge { .. } => Some("too large to show"),
        FileDiff::ModeOnly { .. } => Some("mode change only"),
        FileDiff::EmptyFile { .. } => Some("empty file"),
        FileDiff::WhitespaceOnly => Some("whitespace only"),
        FileDiff::Submodule { .. } => Some("submodule"),
    }
}

fn selection(
    request_path: &str,
    hunks: &[diff_engine::Hunk],
    chosen: &[bool],
) -> diff_engine::PatchRequest {
    let mut selected_deletes = Vec::new();
    let mut selected_inserts = Vec::new();
    for (hunk, _) in hunks.iter().zip(chosen).filter(|(_, on)| **on) {
        for row in &hunk.rows {
            match row {
                diff_engine::DiffRow::Delete { old, .. } => selected_deletes.push(*old),
                diff_engine::DiffRow::Insert { new, .. } => selected_inserts.push(*new),
                diff_engine::DiffRow::Context { .. } => {}
            }
        }
    }
    diff_engine::PatchRequest {
        path: request_path.to_owned(),
        hunks: hunks.to_vec(),
        selected_deletes,
        selected_inserts,
    }
}

impl AppState {
    pub(crate) fn local_split_commit_blocked(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<Option<String>, GitError> {
        self.handle(repo)?.split_commit_blocked(rev)
    }

    pub(crate) fn local_split_preview(
        &self,
        repo: RepoId,
        rev: &str,
    ) -> Result<SplitPreview, GitError> {
        let handle = self.handle(repo)?;
        let oid = handle.commit_id(rev)?;
        let message = handle.commit_message(&oid)?;
        let files = self
            .split_diffs(repo, &oid)?
            .into_iter()
            .map(|diff| diff.file)
            .collect();
        Ok(SplitPreview { message, files })
    }

    fn split_diffs(&self, repo: RepoId, rev: &str) -> Result<Vec<SplitFileDiff>, GitError> {
        let handle = self.handle(repo)?;
        let oid = handle.commit_id(rev)?;
        let spec = git_engine::DiffSpec::CommitVsParent { oid: oid.clone() };
        let options = options();
        let mut files = Vec::new();
        for entry in handle.commit_files(&oid)? {
            let diff = self.local_diff_file(repo, &spec, &entry.path, &options)?;
            let mut notes: Vec<String> = Vec::new();
            if entry.status == FileStatus::Renamed
                && let Some(old) = &entry.old_path
            {
                notes.push(format!("renamed from {old}"));
            }
            if entry.mode_change.is_some() {
                notes.push("mode changed".to_owned());
            }
            if let Some(note) = note_of(&diff) {
                notes.push(note.to_owned());
            }
            let plain = entry.status == FileStatus::Modified
                && entry.mode_change.is_none()
                && matches!(entry.mode, FileMode::Plain | FileMode::Executable);
            let (hunks, whole) = match diff {
                diff_engine::FileDiff::Text {
                    hunks,
                    converted: None,
                    lossy_encoding: false,
                    ..
                } => {
                    let splittable = plain
                        && hunks.len() > 1
                        && match self.selection_patch(
                            repo,
                            &selection(&entry.path, &hunks, &vec![true; hunks.len()]),
                            &spec,
                            false,
                        ) {
                            Ok(_) => true,
                            Err(err @ GitError::InvalidState(_)) => {
                                tracing::warn!(error = ?err, path = entry.path, "split treats an unpatchable text file as one block");
                                false
                            }
                            Err(err) => return Err(err),
                        };
                    (hunks, !splittable)
                }
                diff_engine::FileDiff::Text { hunks, .. } => (hunks, true),
                _ => (Vec::new(), true),
            };
            let blocks = if whole {
                vec!["Whole file".to_owned()]
            } else {
                hunks.iter().map(|hunk| hunk.header.clone()).collect()
            };
            files.push(SplitFileDiff {
                file: SplitFile {
                    path: entry.path,
                    blocks,
                    whole,
                    note: (!notes.is_empty()).then(|| notes.join(", ")),
                },
                rename_source: entry
                    .old_path
                    .filter(|_| entry.status == FileStatus::Renamed),
                hunks,
            });
        }
        Ok(files)
    }

    pub(crate) fn local_split_commit(
        &self,
        repo: RepoId,
        rev: &str,
        parts: &[SplitPart],
    ) -> Result<(), GitError> {
        if parts.len() < 2 {
            return Err(GitError::InvalidState(
                "a split needs at least two commits".to_owned(),
            ));
        }
        let handle = self.handle(repo)?;
        let oid = handle.commit_id(rev)?;
        if let Some(reason) = handle.split_commit_blocked(&oid)? {
            return Err(GitError::InvalidState(reason));
        }
        let files = self.split_diffs(repo, &oid)?;
        let owners = owners(files.iter().map(|diff| &diff.file), parts)?;
        let mut nonempty = vec![false; parts.len()];
        for &part in owners.iter().flatten() {
            nonempty[part] = true;
        }
        if let Some(empty) = nonempty.iter().position(|has_blocks| !has_blocks) {
            return Err(GitError::InvalidState(format!(
                "commit {} of the split needs a block",
                empty + 1
            )));
        }
        let spec = git_engine::DiffSpec::CommitVsParent { oid: oid.clone() };
        let mut cuts = Vec::with_capacity(parts.len());
        for upto in 0..parts.len() {
            let mut cut = git_engine::SplitCut::default();
            for (diff, owned) in files.iter().zip(&owners) {
                let file = &diff.file;
                let taken: Vec<bool> = owned.iter().map(|part| *part <= upto).collect();
                if taken.iter().all(|on| *on) {
                    if let Some(old) = &diff.rename_source {
                        cut.paths.push(old.clone());
                    }
                    cut.paths.push(file.path.clone());
                } else if taken.iter().any(|on| *on) {
                    let request = selection(&file.path, &diff.hunks, &taken);
                    let mut patch = self.selection_patch(repo, &request, &spec, false)?;
                    if !patch.ends_with('\n') {
                        patch.push('\n');
                    }
                    cut.patch.push_str(&patch);
                }
            }
            cuts.push(cut);
        }
        let messages: Vec<String> = parts.iter().map(|part| part.message.clone()).collect();

        let _quiet = self.quiet(repo);
        let before = crate::safety::Before::head(&handle)?;
        let label = format!("Split {} into {} commits", short(&oid), parts.len());
        match handle.split_commit(&oid, &cuts, &messages) {
            Ok(moved) => {
                self.record(
                    repo,
                    label,
                    crate::Recovery::Moved {
                        name: moved.branch,
                        oid: moved.from,
                        after: Some(moved.to),
                        stacked: Vec::new(),
                    },
                );
                Ok(())
            }
            Err(err) => {
                let result = Err(err);
                self.record_move(repo, &handle, before, label, &result);
                result
            }
        }
    }
}

fn owners<'a>(
    files: impl IntoIterator<Item = &'a SplitFile>,
    parts: &[SplitPart],
) -> Result<Vec<Vec<usize>>, GitError> {
    let last = parts.len() - 1;
    let files: Vec<_> = files.into_iter().collect();
    let index: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(at, file)| (file.path.as_str(), at))
        .collect();
    let mut owners: Vec<Vec<Option<usize>>> = files
        .iter()
        .map(|file| vec![None; file.blocks.len()])
        .collect();
    for (part, each) in parts.iter().enumerate() {
        for block in &each.blocks {
            let Some(&at) = index.get(block.path.as_str()) else {
                return Err(GitError::InvalidState(format!(
                    "{} is not one of the files this commit changed",
                    block.path
                )));
            };
            let slots = &mut owners[at];
            let range = match block.hunk {
                None => 0..slots.len(),
                Some(_) if files[at].whole => {
                    return Err(GitError::InvalidState(format!(
                        "{} moves as one block, not by hunks",
                        block.path
                    )));
                }
                Some(hunk) => {
                    let hunk = hunk as usize;
                    if hunk >= slots.len() {
                        return Err(GitError::InvalidState(format!(
                            "{} has no block {}",
                            block.path,
                            hunk + 1
                        )));
                    }
                    hunk..hunk + 1
                }
            };
            for slot in &mut slots[range] {
                if slot.is_some_and(|owner| owner != part) {
                    return Err(GitError::InvalidState(format!(
                        "a block of {} is given to two commits",
                        block.path
                    )));
                }
                *slot = Some(part);
            }
        }
    }
    Ok(owners
        .into_iter()
        .map(|slots| slots.into_iter().map(|slot| slot.unwrap_or(last)).collect())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, hunks: usize, whole: bool) -> SplitFile {
        SplitFile {
            path: path.to_owned(),
            blocks: vec![String::new(); if whole { 1 } else { hunks }],
            whole,
            note: None,
        }
    }

    fn part(blocks: &[(&str, Option<u32>)]) -> SplitPart {
        SplitPart {
            message: "m".to_owned(),
            blocks: blocks
                .iter()
                .map(|(path, hunk)| SplitBlock {
                    path: (*path).to_owned(),
                    hunk: *hunk,
                })
                .collect(),
        }
    }

    #[test]
    fn unassigned_blocks_go_to_the_last_commit() {
        let files = [file("a", 2, false), file("b", 0, true)];
        let parts = [part(&[("a", Some(1))]), part(&[]), part(&[])];
        assert_eq!(owners(&files, &parts).unwrap(), [vec![2, 0], vec![2]]);
    }

    #[test]
    fn a_whole_file_named_without_a_hunk_takes_every_block() {
        let files = [file("a", 3, false)];
        let parts = [part(&[("a", None)]), part(&[])];
        assert_eq!(owners(&files, &parts).unwrap(), [vec![0, 0, 0]]);
    }

    #[test]
    fn a_block_in_two_commits_is_refused() {
        let files = [file("a", 2, false)];
        let parts = [part(&[("a", Some(0))]), part(&[("a", Some(0))])];
        assert!(owners(&files, &parts).is_err());
    }

    #[test]
    fn a_hunk_of_a_whole_file_is_refused() {
        let files = [file("a", 2, true)];
        assert!(owners(&files, &[part(&[("a", Some(0))]), part(&[])]).is_err());
    }

    #[test]
    fn an_unknown_path_or_hunk_is_refused() {
        let files = [file("a", 2, false)];
        assert!(owners(&files, &[part(&[("b", None)]), part(&[])]).is_err());
        assert!(owners(&files, &[part(&[("a", Some(2))]), part(&[])]).is_err());
    }
}
