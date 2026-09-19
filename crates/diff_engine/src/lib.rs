use serde::Serialize;

mod eol;

pub use eol::{EolInfo, LineEnding, detect_line_ending, normalize_line_endings};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub enum Algorithm {
    #[default]
    Histogram,
    Myers,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub enum Whitespace {
    #[default]
    None,
    Trailing,
    All,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DiffOptions {
    pub algorithm: Algorithm,
    pub context_lines: u32,
    pub ignore_whitespace: Whitespace,
    pub ignore_blank_lines: bool,
    pub word_diff: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::default(),
            context_lines: 3,
            ignore_whitespace: Whitespace::default(),
            ignore_blank_lines: false,
            word_diff: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind")]
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
    },
    Insert {
        new: u32,
        text: String,
        inline: Vec<(u32, u32)>,
    },
    Collapsed {
        count: u32,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub rows: Vec<DiffRow>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind")]
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
        old_size: u64,
        new_size: u64,
    },
    Image {
        old_size: u64,
        new_size: u64,
        mime: String,
    },
    TooLarge {
        size: u64,
    },
    Unchanged,
}

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
pub enum DiffError {
    #[error("failed to decode {path}")]
    Decode { path: String },
    #[error("internal error: {0}")]
    Internal(String),
}
