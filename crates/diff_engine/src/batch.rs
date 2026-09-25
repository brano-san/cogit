use crate::{DiffOptions, FileDiff};
use rayon::prelude::*;
use serde::Serialize;

/// One file's two sides, as the caller read them out of the object database.
#[derive(Debug, Clone)]
pub struct FileInput {
    pub path: String,
    pub old: Vec<u8>,
    pub new: Vec<u8>,
}

/// Named rather than a tuple: a positional pair crossing IPC reads as `[string, FileDiff]`
/// on the other side, and nothing there says which half is which.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffEntry {
    pub path: String,
    pub diff: FileDiff,
}

/// The whole per-file pipeline in one place: diff, language hint, hunk headers, moves.
#[must_use]
pub fn diff_one(path: &str, old: &[u8], new: &[u8], options: &DiffOptions) -> FileDiff {
    let mut diff = crate::diff_bytes(old, new, options);
    if let FileDiff::Text { language, .. } = &mut diff {
        *language = crate::language_for_path(path);
        crate::with_hunk_context(&mut diff, &String::from_utf8_lossy(old));
    }
    if options.detect_moves {
        crate::detect_moves(&mut diff);
    }
    diff
}

/// Parallel **by file**, never inside one: a single file's diff does not split well, a
/// batch of them does (doc/08-diff-engine.md section 9). Order follows the input.
///
/// The caller must already be off the async runtime. `rayon` inside a Tokio worker blocks
/// it and stalls IPC along with everything else ([INV-01](doc/01-architecture.md)).
#[must_use]
pub fn diff_many(files: Vec<FileInput>, options: &DiffOptions) -> Vec<FileDiffEntry> {
    let mut entries: Vec<FileDiffEntry> = files
        .into_par_iter()
        .map(|file| FileDiffEntry {
            diff: diff_one(&file.path, &file.old, &file.new, options),
            path: file.path,
        })
        .collect();

    // Sequential, and only once the parallel pass is done: a block that left one file for
    // another is invisible from inside either of them.
    if options.detect_moves {
        crate::link_moves_across_files(&mut entries);
    }
    entries
}
