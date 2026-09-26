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
        let source = handle.rename_source(spec, path)?;
        let old_path = source.as_deref().unwrap_or(path);
        if let Some(diff) = too_large(handle.side_sizes_from(spec, old_path, path)?) {
            return Ok(named(diff, &handle, spec, (old_path, path)));
        }
        let (old, new) = handle.diff_sides_from(spec, old_path, path)?;
        if old.is_none() && new.is_none() {
            return pointer_diff(&handle, spec, path);
        }

        let diff = diff_engine::diff_one_as(
            path,
            old.as_deref().unwrap_or_default(),
            new.as_deref().unwrap_or_default(),
            options,
            &content(handle.diff_attributes().content(path)),
        );
        let present = (old.is_some(), new.is_some());
        let diff = explain_unchanged(diff, &handle, spec, (old_path, path), present);
        Ok(named(
            mark_absent(diff, present),
            &handle,
            spec,
            (old_path, path),
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
        let mut attributes = handle.diff_attributes();
        // Where the old side is and which sides exist, for a file whose bytes are equal.
        let mut sides = std::collections::HashMap::new();
        let mut inputs = Vec::with_capacity(paths.len());
        // `None` stands for the next text diff, so the answer keeps the order it was asked in.
        let mut slots = Vec::with_capacity(paths.len());

        for path in paths {
            // Between files, never inside one: `imara-diff` cannot be interrupted part-way.
            if !self.diff_request_is_current(repo, request) {
                return Ok(DiffBatch::Superseded);
            }

            let source = handle.rename_source(spec, path)?;
            let old_path = source.as_deref().unwrap_or(path);
            if let Some(diff) = too_large(handle.side_sizes_from(spec, old_path, path)?) {
                slots.push(Some(diff_engine::FileDiffEntry {
                    path: path.clone(),
                    diff: named(diff, &handle, spec, (old_path, path)),
                }));
                continue;
            }
            let (old, new) = handle.diff_sides_from(spec, old_path, path)?;
            if old.is_none() && new.is_none() {
                slots.push(Some(diff_engine::FileDiffEntry {
                    path: path.clone(),
                    diff: pointer_diff(&handle, spec, path)?,
                }));
                continue;
            }
            sides.insert(
                path.clone(),
                (old_path.to_owned(), (old.is_some(), new.is_some())),
            );
            slots.push(None);
            inputs.push(diff_engine::FileInput {
                path: path.clone(),
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
                content: content(attributes.content(path)),
            });
        }

        let mut texts = diff_engine::diff_many(inputs, options)
            .into_iter()
            .map(|entry| {
                let diff = match sides.get(&entry.path) {
                    Some((old_path, present)) => {
                        let diff = explain_unchanged(
                            entry.diff,
                            &handle,
                            spec,
                            (old_path, &entry.path),
                            *present,
                        );
                        named(
                            mark_absent(diff, *present),
                            &handle,
                            spec,
                            (old_path, &entry.path),
                        )
                    }
                    None => entry.diff,
                };
                diff_engine::FileDiffEntry {
                    diff,
                    path: entry.path,
                }
            });
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
        let (old, new) = self.handle(repo)?.patch_sides(spec, &request.path)?;
        // A stand-in character only means a lost byte where a side is not UTF-8: the same
        // character written in valid UTF-8 is just text (an icon font's, say).
        let lossy = |side: &Option<Vec<u8>>| {
            side.as_deref()
                .is_some_and(|b| std::str::from_utf8(b).is_err())
        };
        if diff_engine::carries_undecoded_bytes(request) && (lossy(&old) || lossy(&new)) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{} is not valid UTF-8: stage or discard it whole, not line by line",
                request.path
            )));
        }
        let shape = diff_engine::PatchShape {
            reverse,
            old_exists: old.is_some(),
            new_exists: new.is_some(),
        };
        let sides = diff_engine::PatchSides {
            old: old.as_deref().unwrap_or_default(),
            new: new.as_deref().unwrap_or_default(),
        };
        diff_engine::build_patch(request, shape, sides).map_err(|err| match err {
            diff_engine::PatchError::NothingSelected => {
                git_engine::GitError::InvalidState(err.to_string())
            }
            _ => git_engine::GitError::InvalidState(format!("{}: {err}", request.path)),
        })
    }

    pub fn merge_preview(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<Vec<diff_engine::Region>, git_engine::GitError> {
        let sides = self.handle(repo)?.conflict_sides(path)?;
        if !sides.is_text() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{path} is binary or not UTF-8: take one side whole"
            )));
        }
        // An absent side merged as an empty file reads as "every line deleted", and taking
        // it would keep the file, empty, instead of deleting it.
        if sides.ours.is_none() || sides.theirs.is_none() {
            return Err(git_engine::GitError::InvalidState(format!(
                "{path} was deleted on one side: keep one side or delete the file"
            )));
        }
        let sides = sides.to_text();
        Ok(diff_engine::merge3_with_syntax(
            sides.base.as_deref().unwrap_or_default(),
            sides.ours.as_deref().unwrap_or_default(),
            sides.theirs.as_deref().unwrap_or_default(),
            diff_engine::merge_grammar_for_path(path),
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

/// Equal bytes, and yet the file list shows the file as changed: its mode changed, or an
/// empty file came or went.
fn explain_unchanged(
    diff: diff_engine::FileDiff,
    handle: &git_engine::RepoHandle,
    spec: &git_engine::DiffSpec,
    (old_path, path): (&str, &str),
    present: (bool, bool),
) -> diff_engine::FileDiff {
    use diff_engine::FileDiff;
    if !matches!(diff, FileDiff::Unchanged) {
        return diff;
    }
    match present {
        (false, true) => return FileDiff::EmptyFile { added: true },
        (true, false) => return FileDiff::EmptyFile { added: false },
        _ => {}
    }
    match handle.side_modes(spec, old_path, path) {
        (Some(old_mode), Some(new_mode)) if old_mode != new_mode => {
            FileDiff::ModeOnly { old_mode, new_mode }
        }
        _ => diff,
    }
}

/// `binary` or `-diff` in `.gitattributes` shows the file as git shows it, never as lines to
/// stage; `text` or `diff` shows lines whatever the bytes hold (R-531).
fn content(said: git_engine::DiffContent) -> diff_engine::Content {
    match said {
        git_engine::DiffContent::Detect => diff_engine::Content::Detect,
        git_engine::DiffContent::Text => diff_engine::Content::Text,
        git_engine::DiffContent::Binary(name) => diff_engine::Content::Binary(name.to_owned()),
    }
}

/// The engine sees bytes, and an absent side reads as empty: a summary says it is absent.
fn mark_absent(
    mut diff: diff_engine::FileDiff,
    (old_is, new_is): (bool, bool),
) -> diff_engine::FileDiff {
    use diff_engine::FileDiff;
    if let FileDiff::Binary { old, new, .. } | FileDiff::TooLarge { old, new, .. } = &mut diff {
        if !old_is {
            *old = None;
        }
        if !new_is {
            *new = None;
        }
    }
    diff
}

/// A summary names each side by its object id. Only a summary: finding the id of a working
/// file costs a read and a hash. Unnamed is still a summary, so a failure is logged.
fn named(
    mut diff: diff_engine::FileDiff,
    handle: &git_engine::RepoHandle,
    spec: &git_engine::DiffSpec,
    (old_path, path): (&str, &str),
) -> diff_engine::FileDiff {
    use diff_engine::FileDiff;
    let (FileDiff::Binary { old, new, .. } | FileDiff::TooLarge { old, new, .. }) = &mut diff
    else {
        return diff;
    };
    let (old_id, new_id) = match handle.side_ids_from(spec, old_path, path) {
        Ok(ids) => ids,
        Err(err) => {
            tracing::error!(error = ?err, path, context = "naming the sides of a summarised diff");
            (None, None)
        }
    };
    for (side, id) in [(old, old_id), (new, new_id)] {
        if let Some(side) = side {
            side.id = id;
        }
    }
    diff
}

/// Past anything the diff shows — an image's cap: then neither side is read, or a
/// multi-gigabyte file goes into memory only to be summarised. Text past its own limit is
/// told apart once read.
fn too_large((old, new): (Option<u64>, Option<u64>)) -> Option<diff_engine::FileDiff> {
    if old.unwrap_or(0).max(new.unwrap_or(0)) <= diff_engine::MAX_IMAGE_BYTES {
        return None;
    }
    let side = |size: Option<u64>| size.map(|size| diff_engine::BlobSide { size, id: None });
    Some(diff_engine::FileDiff::TooLarge {
        old: side(old),
        new: side(new),
        // Unread, it is not known to be an image, and past this it is past the text limit.
        limit: diff_engine::MAX_TEXT_BYTES,
    })
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
            in_index: pointer.in_index,
        }),
        None => match spec {
            git_engine::DiffSpec::WorkTreeVsIndex
            | git_engine::DiffSpec::CommitVsWorkTree { .. } => handle
                .folder_on_disk(path)
                .map(|repository| diff_engine::FileDiff::Folder { repository })
                .ok_or_else(|| absent(path)),
            _ => Err(absent(path)),
        },
    }
}

fn absent(path: &str) -> git_engine::GitError {
    git_engine::GitError::InvalidState(format!("{path} is absent from both sides of the diff"))
}
