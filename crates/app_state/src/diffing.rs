//! Turning two sides of a file into something the Diff panel can draw, and applying a
//! selection back. The engine stays a pure function; the state lives here.

use crate::{AppState, DiffBatch, Recovery, RepoId, newest_diff_request};

impl AppState {
    pub fn diff_file(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
        options: &diff_engine::DiffOptions,
    ) -> Result<diff_engine::FileDiff, git_engine::GitError> {
        let (old, new) = self.handle(repo)?.diff_sides(spec, path)?;
        if old.is_none() && new.is_none() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{path} is absent from both sides of the diff"
            )));
        }

        Ok(diff_engine::diff_one(
            path,
            old.as_deref().unwrap_or_default(),
            new.as_deref().unwrap_or_default(),
            options,
        ))
    }

    /// Every file of a commit in one call.
    ///
    /// The object reads stay sequential — a `gix` repository is not shared across threads
    /// — and only the diffing is parallel, which is where the time goes anyway
    /// (doc/08-diff-engine.md section 9).
    ///
    /// Blocking by design; the Tauri layer wraps it in `spawn_blocking`, and `rayon` must
    /// never be entered from an async task ([INV-01](doc/01-architecture.md)).
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

        for path in paths {
            // Between files, never inside one: there is no way to interrupt `imara-diff`
            // part-way, and a single file is short enough that it does not matter.
            if !self.diff_request_is_current(repo, request) {
                return Ok(DiffBatch::Superseded);
            }

            let (old, new) = handle.diff_sides(spec, path)?;
            if old.is_none() && new.is_none() {
                return Err(git_engine::GitError::InvalidState(format!(
                    "{path} is absent from both sides of the diff"
                )));
            }
            inputs.push(diff_engine::FileInput {
                path: path.clone(),
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
            });
        }

        let files = diff_engine::diff_many(inputs, options);
        if self.diff_request_is_current(repo, request) {
            Ok(DiffBatch::Ready { files })
        } else {
            Ok(DiffBatch::Superseded)
        }
    }

    /// Records `request` as the newest for `repo` and reports whether it still is. An
    /// older number never displaces a newer one, so responses cannot arrive out of order.
    fn claim_diff_request(&self, repo: RepoId, request: u32) -> bool {
        let key = (std::ptr::from_ref(self) as usize, repo);
        let mut newest = newest_diff_request().write();
        match newest.get(&key) {
            Some(&seen) if seen > request => false,
            _ => {
                newest.insert(key, request);
                true
            }
        }
    }

    fn diff_request_is_current(&self, repo: RepoId, request: u32) -> bool {
        let key = (std::ptr::from_ref(self) as usize, repo);
        newest_diff_request().read().get(&key) == Some(&request)
    }

    /// Builds the patch and applies it in one step: the two halves must never drift
    /// apart, and a half-applied selection is exactly the damage R-04 warns about.
    pub fn stage_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
        reverse: bool,
    ) -> Result<(), git_engine::GitError> {
        let Some(patch) = diff_engine::build_patch(request) else {
            return Err(git_engine::GitError::InvalidState(
                "nothing selected".to_owned(),
            ));
        };
        self.quiet(repo);
        self.handle(repo)?.apply_patch(&patch, reverse)
    }

    /// Throws the selected lines away in the working tree.
    ///
    /// The patch that was reversed is kept, so undo applies it again and puts back exactly
    /// those lines. R-106 recorded the earlier answer — that a line-level discard had
    /// nowhere to restore from — and it no longer holds.
    pub fn discard_selection(
        &self,
        repo: RepoId,
        request: &diff_engine::PatchRequest,
    ) -> Result<(), git_engine::GitError> {
        let Some(patch) = diff_engine::build_patch(request) else {
            return Err(git_engine::GitError::InvalidState(
                "nothing selected".to_owned(),
            ));
        };
        self.quiet(repo);
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

    /// The three sides already merged: the view needs regions to count and step through,
    /// not a file with markers in it.
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

    /// Both sides of an image as `data:` URLs; `None` on a side the file is absent from.
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
