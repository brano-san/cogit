use serde::{Deserialize, Serialize};

mod batch;
mod eol;
mod headers;
mod images;
mod language;
mod merge;
mod moves;
mod patch;
mod syntax;
mod text;
mod words;

pub use batch::{FileDiffEntry, FileInput, diff_many, diff_one};
pub use eol::{EolInfo, LineEnding, detect_line_ending, normalize_line_endings};
pub use headers::with_hunk_context;
pub use images::{base64, data_url, image_mime};
pub use language::language_for_path;
pub use merge::{Origin, Region, merge3, merge3_with_syntax};
pub use moves::{MIN_MOVED_LINES, detect_moves, link_moves_across_files};
pub use patch::{PatchRequest, build_patch};
pub use text::{MAX_TEXT_BYTES, diff_bytes, diff_text};
pub use words::{Spans, block_is_comparable, inline_spans};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Algorithm {
    #[default]
    Histogram,
    Myers,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Whitespace {
    #[default]
    None,
    Trailing,
    All,
}

#[derive(Debug, Clone, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffOptions {
    pub algorithm: Algorithm,
    pub context_lines: u32,
    pub ignore_whitespace: Whitespace,
    pub ignore_blank_lines: bool,
    pub word_diff: bool,
    pub detect_moves: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::default(),
            context_lines: 3,
            ignore_whitespace: Whitespace::default(),
            ignore_blank_lines: false,
            word_diff: true,
            detect_moves: true,
        }
    }
}

/// Where the other end of a move is. Two different facts, drawn two different ways: a
/// block that travelled inside its file, and one that left it for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MoveScope {
    WithinFile,
    AcrossFiles,
}

#[derive(Debug, Clone, Serialize, specta::Type, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiffRow {
    Context {
        old: u32,
        new: u32,
        text: String,
    },
    Delete {
        old: u32,
        text: String,
        inline: Vec<(u32, u32)>,
        #[serde(default)]
        moved: bool,
        /// Both ends of one move carry the same number, so the UI can draw the pair.
        #[serde(default)]
        move_id: Option<u32>,
        #[serde(default)]
        move_scope: Option<MoveScope>,
        /// The file ends on this row without a final newline; a unified diff prints
        /// `\ No newline at end of file` underneath it.
        #[serde(default)]
        no_newline: bool,
    },
    Insert {
        new: u32,
        text: String,
        inline: Vec<(u32, u32)>,
        #[serde(default)]
        moved: bool,
        #[serde(default)]
        move_id: Option<u32>,
        #[serde(default)]
        move_scope: Option<MoveScope>,
        #[serde(default)]
        no_newline: bool,
    },
    Collapsed {
        count: u32,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub rows: Vec<DiffRow>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FileDiff {
    Text {
        hunks: Vec<Hunk>,
        eol: EolInfo,
        lossy_encoding: bool,
        /// Language hint for Lezer. Highlighting itself is a frontend concern (INV-01).
        language: Option<String>,
        /// Lines on each side, so the view can say how many follow the last hunk.
        old_total: u32,
        new_total: u32,
    },
    EolOnly {
        from: LineEnding,
        to: LineEnding,
    },
    Binary {
        #[specta(type = specta_typescript::Number)]
        old_size: u64,
        #[specta(type = specta_typescript::Number)]
        new_size: u64,
    },
    Image {
        #[specta(type = specta_typescript::Number)]
        old_size: u64,
        #[specta(type = specta_typescript::Number)]
        new_size: u64,
        mime: String,
    },
    TooLarge {
        #[specta(type = specta_typescript::Number)]
        size: u64,
    },
    Unchanged,
    /// Nothing but whitespace changed, and the active option hides it. Told apart from
    /// `Unchanged` so the UI can say the diff is being filtered (T7.10).
    WhitespaceOnly,
    /// A gitlink: what changed is which commit the parent records, not any file. A
    /// submodule that was never checked out has nothing else to show, and that is a
    /// normal state of a repository rather than a broken one (doc/12-risks.md, R-139).
    Submodule {
        /// The commit the parent records now, and the one it recorded before.
        recorded: String,
        previous: Option<String>,
        /// False when the submodule's own repository is not on disk.
        checked_out: bool,
    },
}
