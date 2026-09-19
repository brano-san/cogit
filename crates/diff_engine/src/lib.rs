//! Block and word-level diffing, plus syntactic three-way merge.
//!
//! Pipeline and rules: `doc/08-diff-engine.md`.
//!
//! **Before writing any `imara-diff` code, open docs.rs for the exact version in
//! `Cargo.lock`.** The 0.2 API shares almost nothing with the 0.1 examples that
//! dominate search results and model training data.

use serde::Serialize;

mod eol;

pub use eol::{EolInfo, LineEnding, detect_line_ending, normalize_line_endings};

/// Line-level diff algorithm. Histogram is the default: it produces noticeably more
/// readable blocks on real code than Myers, and matches `git diff --histogram`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub enum Algorithm {
    #[default]
    Histogram,
    Myers,
}

/// How whitespace differences are treated.
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
    /// Enables the `similar`-based intra-line pass.
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

/// One rendered row of a diff.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind")]
pub enum DiffRow {
    Context {
        old: u32,
        new: u32,
        text: String,
    },
    /// `inline` holds byte ranges within `text`, not character indices — the frontend
    /// must convert them for CodeMirror. Multi-byte text makes this an easy bug.
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
    /// Unified-diff header. Reused when generating patches for partial staging.
    pub header: String,
    pub rows: Vec<DiffRow>,
}

/// Result of diffing one file.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind")]
pub enum FileDiff {
    Text {
        hunks: Vec<Hunk>,
        eol: EolInfo,
        /// Set when the file could not be decoded cleanly. The user must be told.
        lossy_encoding: bool,
        /// Language hint for Lezer. Highlighting itself is a frontend concern (INV-01).
        language: Option<String>,
    },
    /// Only the line endings differ. Shown with a dedicated notice rather than as
    /// a whole-file change.
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
