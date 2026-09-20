use serde::{Deserialize, Serialize};

mod batch;
mod eol;
mod headers;
mod images;
mod language;
mod moves;
mod patch;
mod text;
mod words;

pub use batch::{FileDiffEntry, FileInput, diff_many, diff_one};
pub use eol::{EolInfo, LineEnding, detect_line_ending, normalize_line_endings};
pub use headers::with_hunk_context;
pub use images::{data_url, image_mime};
pub use language::language_for_path;
pub use moves::{MIN_MOVED_LINES, detect_moves};
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
    },
    Insert {
        new: u32,
        text: String,
        inline: Vec<(u32, u32)>,
        #[serde(default)]
        moved: bool,
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
}

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DiffError {
    #[error("failed to decode {path}")]
    Decode { path: String },
    #[error("internal error: {0}")]
    Internal(String),
}
