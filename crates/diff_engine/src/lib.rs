use serde::{Deserialize, Serialize};

mod batch;
mod binary;
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

pub use batch::{FileDiffEntry, FileInput, diff_many, diff_one, diff_one_as};
pub use binary::invalid_character;
pub use eol::{EolInfo, LineEnding, detect_line_ending, normalize_line_endings};
pub use headers::with_hunk_context;
pub use images::{base64, data_url, image_mime};
pub use language::{MAX_HIGHLIGHT_LINES, highlighted, language_for_path, merge_grammar_for_path};
pub use merge::{Origin, Region, merge3, merge3_with_syntax};
pub use moves::{MIN_MOVED_LINES, detect_moves, link_moves_across_files};
pub use patch::{
    PatchError, PatchRequest, PatchShape, PatchSides, build_patch, carries_undecoded_bytes,
};
pub use text::{MAX_IMAGE_BYTES, MAX_TEXT_BYTES, diff_bytes, diff_bytes_as, diff_text};
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
    pub word_diff: bool,
    pub detect_moves: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::default(),
            context_lines: 3,
            ignore_whitespace: Whitespace::default(),
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
        /// The last line of both sides, neither ending in a newline.
        #[serde(default)]
        no_newline: bool,
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

/// One side of a file shown as a summary: its size and the object id git gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlobSide {
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    /// `None` until the caller that read the side fills it in, and for a working-tree file
    /// too large to read.
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DiffSide {
    Old,
    New,
}

/// Why a file is shown as binary, in the terms SmartGit words it (R-531).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BinaryCause {
    /// `.gitattributes` says so: `binary`, or `-diff`.
    Attribute { name: String },
    /// A control character text does not hold, first found on `side`; `line` and
    /// `position` count from 1, `position` in characters.
    Character {
        code: u8,
        line: u32,
        position: u32,
        side: DiffSide,
    },
}

/// What `.gitattributes` says about reading a path as text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Content {
    /// Nothing: the bytes decide.
    #[default]
    Detect,
    /// `text` or `diff`: lines, whatever the bytes hold.
    Text,
    /// `binary` or `-diff`, by that name.
    Binary(String),
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
        /// Each side whole, as the rows quote it, for the highlighter: the hunks alone parse
        /// as broken code (R-530). Only for a language the frontend has a parser for, and a
        /// side of at most `MAX_HIGHLIGHT_LINES` lines.
        old_text: Option<String>,
        new_text: Option<String>,
    },
    EolOnly {
        from: LineEnding,
        to: LineEnding,
    },
    /// Shown as a summary, never as lines; `None` on a side the file is absent from.
    Binary {
        old: Option<BlobSide>,
        new: Option<BlobSide>,
        cause: BinaryCause,
    },
    Image {
        #[specta(type = specta_typescript::Number)]
        old_size: u64,
        #[specta(type = specta_typescript::Number)]
        new_size: u64,
        mime: String,
    },
    /// A side of `limit` bytes or more, summarised as a binary file is.
    TooLarge {
        old: Option<BlobSide>,
        new: Option<BlobSide>,
        #[specta(type = specta_typescript::Number)]
        limit: u64,
    },
    Unchanged,
    /// The same content, and only the mode changed: `100644` to `100755`, as git prints it.
    ModeOnly {
        old_mode: String,
        new_mode: String,
    },
    /// A file with no content, added or deleted: both sides read as nothing, yet the file
    /// is there on one of them only.
    EmptyFile {
        added: bool,
    },
    /// Nothing but whitespace changed, and the active option hides it. Told apart from
    /// `Unchanged` so the UI can say the diff is being filtered (T7.10).
    WhitespaceOnly,
    /// A folder on disk Git tracks nothing in: one untracked entry, or a repository cloned
    /// inside this one without being its submodule. A normal state, not a missing path.
    Folder {
        repository: bool,
    },
    /// A gitlink: what changed is which commit the parent records, not any file. A
    /// submodule that was never checked out has nothing else to show, and that is a
    /// normal state of a repository rather than a broken one (doc/12-risks.md, R-139).
    Submodule {
        /// The commit the parent records now, `None` where it removed the submodule, and
        /// the one it recorded before.
        recorded: Option<String>,
        previous: Option<String>,
        /// False when the submodule's own repository is not on disk.
        checked_out: bool,
        /// Whether the index has the gitlink: without it there is nothing to initialise.
        in_index: bool,
    },
}
