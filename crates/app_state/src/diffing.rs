//! Two sides of a file turned into what the Diff panel draws, and a selection applied back.

use crate::{AppState, DiffBatch, Recovery, RepoId};

impl AppState {
    pub fn diff_file(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
        options: &diff_engine::DiffOptions,
    ) -> Result<diff_engine::FileDiff, git_engine::GitError> {
        let handle = self.handle(repo)?;
        let (old, new) = handle.diff_sides(spec, path)?;
        if old.is_none() && new.is_none() {
            return pointer_diff(&handle, spec, path);
        }

        Ok(diff_engine::diff_one(
            path,
            old.as_deref().unwrap_or_default(),
            new.as_deref().unwrap_or_default(),
            options,
        ))
    }

    /// Every file of a commit in one call. Blocking: reads are sequential, only the diffing
    /// is parallel, and the caller wraps it in `spawn_blocking` (doc/08-diff-engine.md §9).
    pub fn diff_files(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        paths: &[String],
        options: &diff_engine::DiffOptions,
        request: u32,
    ) -> Result<DiffBatch, git_engine::GitError> {
        if !self.claim_diff_request(repo, request) {
            return Ok(DiffBatch::Superseded);
        }

        let handle = self.handle(repo)?;
        let mut inputs = Vec::with_capacity(paths.len());
        // `None` stands for the next text diff, so the answer keeps the order it was asked in.
        let mut slots = Vec::with_capacity(paths.len());

        for path in paths {
            // Between files, never inside one: `imara-diff` cannot be interrupted part-way.
            if !self.diff_request_is_current(repo, request) {
                return Ok(DiffBatch::Superseded);
            }

            let (old, new) = handle.diff_sides(spec, path)?;
            if old.is_none() && new.is_none() {
                slots.push(Some(diff_engine::FileDiffEntry {
                    path: path.clone(),
                    diff: pointer_diff(&handle, spec, path)?,
                }));
                continue;
            }
            slots.push(None);
            inputs.push(diff_engine::FileInput {
                path: path.clone(),
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
            });
        }

        let mut texts = diff_engine::diff_many(inputs, options).into_iter();
        let files = slots
            .into_iter()
            .filter_map(|slot| slot.or_else(|| texts.next()))
            .collect();
        if self.diff_request_is_current(repo, request) {
            Ok(DiffBatch::Ready { files })
        } else {
            Ok(DiffBatch::Superseded)
        }
    }

    /// An older request never displaces a newer one, so responses cannot arrive out of order.
    fn claim_diff_request(&self, repo: RepoId, request: u32) -> bool {
        let mut newest = self.newest_diff.write();
        match newest.get(&repo) {
            Some(&seen) if seen > request => false,
            _ => {
                newest.insert(repo, request);
                true
            }
        }
    }

    fn diff_request_is_current(&self, repo: RepoId, request: u32) -> bool {
        self.newest_diff.read().get(&repo) == Some(&request)
    }

    /// Built and applied in one step: a half-applied selection is the damage R-04 warns of.
    pub fn stage_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
        reverse: bool,
    ) -> Result<(), git_engine::GitError> {
        // Unstage is the reverse of the diff of the index against HEAD; Stage goes forward
        // on the diff of the working tree against the index.
        let spec = if reverse {
            git_engine::DiffSpec::IndexVsHead
        } else {
            git_engine::DiffSpec::WorkTreeVsIndex
        };
        let patch = self.selection_patch(repo, request, &spec, reverse)?;
        let _quiet = self.quiet(repo);
        self.handle(repo)?.apply_patch(&patch, reverse)
    }

    /// The reversed patch is journalled, so undo puts back exactly those lines (R-106).
    pub fn discard_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
    ) -> Result<(), git_engine::GitError> {
        let spec = git_engine::DiffSpec::WorkTreeVsIndex;
        let patch = self.selection_patch(repo, request, &spec, true)?;
        let _quiet = self.quiet(repo);
        self.handle(repo)?
            .apply_patch_to(&patch, true, git_engine::PatchTarget::WorkTree)?;
        self.record(
            repo,
            format!("Discard lines in {}", request.path),
            Recovery::Patch {
                path: request.path.clone(),
                patch,
            },
        );
        Ok(())
    }

    /// Whether the file is there on each side decides `/dev/null` in the envelope; the
    /// hunks cannot tell a new file from lines added to the top of an empty-context diff.
    fn selection_patch(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
        spec: &git_engine::DiffSpec,
        reverse: bool,
    ) -> Result<String, git_engine::GitError> {
        let (old, new) = self.handle(repo)?.diff_sides(spec, &request.path)?;
        let shape = diff_engine::PatchShape {
            reverse,
            old_exists: old.is_some(),
            new_exists: new.is_some(),
        };
        diff_engine::build_patch(request, shape)
            .ok_or_else(|| git_engine::GitError::InvalidState("nothing selected".to_owned()))
    }

    pub fn merge_preview(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<Vec<diff_engine::Region>, git_engine::GitError> {
        let sides = self.handle(repo)?.conflict_sides(path)?.to_text();
        let language = diff_engine::language_for_path(path);
        Ok(diff_engine::merge3_with_syntax(
            sides.base.as_deref().unwrap_or_default(),
            sides.ours.as_deref().unwrap_or_default(),
            sides.theirs.as_deref().unwrap_or_default(),
            language.as_deref(),
        ))
    }

    pub fn image_sides(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
    ) -> Result<(Option<String>, Option<String>), git_engine::GitError> {
        let (old, new) = self.handle(repo)?.diff_sides(spec, path)?;
        let encode = |bytes: Option<Vec<u8>>| {
            bytes.and_then(|data| {
                diff_engine::image_mime(&data).map(|mime| diff_engine::data_url(mime, &data))
            })
        };
        Ok((encode(old), encode(new)))
    }
}

/// A gitlink has no content on either side, nor has a missing path: this tells them apart.
fn pointer_diff(
    handle: &git_engine::RepoHandle,
    spec: &git_engine::DiffSpec,
    path: &str,
) -> Result<diff_engine::FileDiff, git_engine::GitError> {
    match handle.submodule_pointer(spec, path)? {
        Some(pointer) => Ok(diff_engine::FileDiff::Submodule {
            recorded: pointer.recorded,
            previous: pointer.previous,
            checked_out: pointer.checked_out,
        }),
        None => Err(git_engine::GitError::InvalidState(format!(
            "{path} is absent from both sides of the diff"
        ))),
    }
}
