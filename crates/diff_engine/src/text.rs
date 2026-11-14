use crate::{
    Algorithm, BinaryCause, BlobSide, Content, DiffOptions, DiffRow, EolInfo, FileDiff, Hunk,
    LineEnding, Spans, Whitespace, block_is_comparable, detect_line_ending, normalize_line_endings,
};
use imara_diff::{Diff, InternedInput, Interner, Token, sources::lines};
use std::borrow::Cow;

/// A side of this many bytes or more is shown as a summary: rendering it would cost more
/// than it tells (R-531).
pub const MAX_TEXT_BYTES: u64 = 1_000_000;

/// The text limit is about rows in the webview; an image is one element, and past this
/// only its data URL is the cost.
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// A fold over this many lines costs more to read than the lines themselves, so they are
/// shown instead: between hunks, above the first and below the last (#16).
const SHOWN_GAP: u32 = 2;

/// Both sides present, sizes known, ids left to the caller that read them.
fn sides(old: &[u8], new: &[u8]) -> (Option<BlobSide>, Option<BlobSide>) {
    let side = |bytes: &[u8]| {
        Some(BlobSide {
            size: bytes.len() as u64,
            id: None,
        })
    };
    (side(old), side(new))
}

#[must_use]
pub fn diff_bytes(old: &[u8], new: &[u8], options: &DiffOptions) -> FileDiff {
    diff_bytes_as(old, new, options, &Content::Detect)
}

/// `diff_bytes` with what `.gitattributes` says about the path.
#[must_use]
pub fn diff_bytes_as(old: &[u8], new: &[u8], options: &DiffOptions, content: &Content) -> FileDiff {
    diff_bytes_sides(old, new, options, content).0
}

/// What a file is shown as when it is not shown as lines.
fn summary(old: &[u8], new: &[u8], content: &Content) -> Option<FileDiff> {
    let size = old.len().max(new.len()) as u64;
    let image = crate::image_mime(old).or_else(|| crate::image_mime(new));
    let too_large = match image {
        Some(_) => size > MAX_IMAGE_BYTES,
        None => size >= MAX_TEXT_BYTES,
    };
    if too_large {
        let (old, new) = sides(old, new);
        return Some(FileDiff::TooLarge {
            old,
            new,
            limit: if image.is_some() {
                MAX_IMAGE_BYTES
            } else {
                MAX_TEXT_BYTES
            },
        });
    }
    if old == new {
        return Some(FileDiff::Unchanged);
    }
    if let Some(mime) = image {
        return Some(FileDiff::Image {
            old_size: old.len() as u64,
            new_size: new.len() as u64,
            mime: mime.to_owned(),
        });
    }
    let cause = match content {
        Content::Binary(name) => Some(BinaryCause::Attribute { name: name.clone() }),
        Content::Text => None,
        Content::Detect => crate::binary::cause(old, new),
    };
    if let Some(cause) = cause {
        let (old, new) = sides(old, new);
        return Some(FileDiff::Binary { old, new, cause });
    }

    None
}

/// Both sides of a text diff as its rows quote them: decoded, with LF line endings.
pub(crate) type TextSides<'a> = (Cow<'a, str>, Cow<'a, str>);

/// `diff_bytes_as`, and for a text diff each side as its rows quote it: decoded, with LF
/// line endings. Kept, so the highlighter's copy is not decoded and normalised again.
pub(crate) fn diff_bytes_sides<'a>(
    old: &'a [u8],
    new: &'a [u8],
    options: &DiffOptions,
    content: &Content,
) -> (FileDiff, Option<TextSides<'a>>) {
    if let Some(summary) = summary(old, new, content) {
        return (summary, None);
    }
    let old_text = decode(old);
    let new_text = decode(new);
    let lossy = matches!(old_text, Cow::Owned(_)) || matches!(new_text, Cow::Owned(_));
    if old_text == new_text {
        return (FileDiff::Unchanged, None);
    }
    let eol = (detect_line_ending(&old_text), detect_line_ending(&new_text));
    let old_text = normalized(old_text);
    let new_text = normalized(new_text);

    let mut diff = diff_normalized(eol, &old_text, &new_text, options);
    if let FileDiff::Text { lossy_encoding, .. } = &mut diff {
        *lossy_encoding = lossy;
    }
    (diff, Some((old_text, new_text)))
}

fn normalized(text: Cow<'_, str>) -> Cow<'_, str> {
    match text {
        Cow::Borrowed(text) => normalize_line_endings(text),
        Cow::Owned(text) => match normalize_line_endings(&text) {
            Cow::Borrowed(_) => Cow::Owned(text),
            Cow::Owned(lf) => Cow::Owned(lf),
        },
    }
}

/// UTF-8 as it is; each byte that is not UTF-8 becomes a character of its own in the
/// Private Use Area (U+F700 plus the byte). One replacement character for all of them made
/// two files that differ only in such bytes compare equal, and a changed file read as
/// unchanged.
fn decode(bytes: &[u8]) -> Cow<'_, str> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Cow::Borrowed(text);
    }
    let mut text = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());
        for &byte in chunk.invalid() {
            text.push(char::from_u32(0xF700 + u32::from(byte)).unwrap_or('\u{FFFD}'));
        }
    }
    Cow::Owned(text)
}

#[must_use]
pub fn diff_text(old: &str, new: &str, options: &DiffOptions) -> FileDiff {
    if old == new {
        return FileDiff::Unchanged;
    }

    let eol = (detect_line_ending(old), detect_line_ending(new));
    diff_normalized(
        eol,
        &normalize_line_endings(old),
        &normalize_line_endings(new),
        options,
    )
}

/// `diff_text` of two sides that differ, already normalised to LF; `eol` is what each had.
fn diff_normalized(
    (old_eol, new_eol): (LineEnding, LineEnding),
    old_text: &str,
    new_text: &str,
    options: &DiffOptions,
) -> FileDiff {
    if old_text == new_text {
        return FileDiff::EolOnly {
            from: old_eol,
            to: new_eol,
        };
    }

    let mut old_lines: Vec<&str> = lines(old_text).collect();
    let mut new_lines: Vec<&str> = lines(new_text).collect();

    let open = OpenEnd {
        old: lacks_final_newline(old_text),
        new: lacks_final_newline(new_text),
    };

    let phantom = !old_text.is_empty() && !new_text.is_empty() && open.old != open.new;
    if phantom {
        if !open.old {
            old_lines.push("");
        } else if !open.new {
            new_lines.push("");
        }
    }

    let mut old_keys: Vec<Cow<'_, str>> = keys(&old_lines, options);
    let mut new_keys: Vec<Cow<'_, str>> = keys(&new_lines, options);
    // A key never holds a newline (lines are split on it), so these match no real line.
    // The empty row standing for a newline only one side has is no blank line of the
    // other: matched with one, it named a line the file does not have and staging failed.
    if phantom {
        let side = if open.old {
            &mut new_keys
        } else {
            &mut old_keys
        };
        if let Some(last) = side.last_mut() {
            *last = Cow::Borrowed("\n");
        }
    }
    // Both sides open: as git compares them, a last line without its newline is not the
    // same line with one on the other side, or staging kept a newline the file lacks.
    if open.old && open.new {
        for side in [&mut old_keys, &mut new_keys] {
            if let Some(last) = side.last_mut() {
                *last = Cow::Owned(format!("{last}\n"));
            }
        }
    }

    let mut interner = Interner::new(old_keys.len() + new_keys.len());
    let before: Vec<Token> = old_keys
        .iter()
        .map(|k| interner.intern(k.as_ref()))
        .collect();
    let after: Vec<Token> = new_keys
        .iter()
        .map(|k| interner.intern(k.as_ref()))
        .collect();
    let input = InternedInput {
        before,
        after,
        interner,
    };

    let mut diff = Diff::compute(algorithm(options.algorithm), &input);
    diff.postprocess_lines(&input);

    let changes: Vec<imara_diff::Hunk> = diff.hunks().collect();
    if changes.is_empty() {
        return if options.ignore_whitespace == Whitespace::None {
            FileDiff::Unchanged
        } else {
            FileDiff::WhitespaceOnly
        };
    }

    let context = options.context_lines as usize;
    let hunks = group(&changes, context)
        .iter()
        .map(|group| build(group, &old_lines, &new_lines, options, open))
        .collect();

    FileDiff::Text {
        hunks,
        eol: EolInfo {
            old: old_eol,
            new: new_eol,
            normalized: old_eol != new_eol,
        },
        lossy_encoding: false,
        language: None,
        old_total: u32::try_from(old_lines.len()).unwrap_or(u32::MAX),
        new_total: u32::try_from(new_lines.len()).unwrap_or(u32::MAX),
        old_text: None,
        new_text: None,
        converted: None,
    }
}

fn algorithm(algorithm: Algorithm) -> imara_diff::Algorithm {
    match algorithm {
        Algorithm::Histogram => imara_diff::Algorithm::Histogram,
        Algorithm::Myers => imara_diff::Algorithm::Myers,
    }
}

/// Which side of the comparison ends without a final newline.
#[derive(Debug, Clone, Copy)]
struct OpenEnd {
    old: bool,
    new: bool,
}

fn keys<'a>(lines: &[&'a str], options: &DiffOptions) -> Vec<Cow<'a, str>> {
    lines.iter().map(|line| key(line, options)).collect()
}

fn lacks_final_newline(text: &str) -> bool {
    !text.is_empty() && !text.ends_with('\n')
}

fn key<'a>(line: &'a str, options: &DiffOptions) -> Cow<'a, str> {
    let body = strip_newline(line);
    match options.ignore_whitespace {
        Whitespace::None => Cow::Borrowed(body),
        Whitespace::Trailing => Cow::Borrowed(body.trim_end()),
        // As `git diff -w`: whitespace does not count even where the other line has none.
        Whitespace::All => Cow::Owned(body.split_whitespace().collect()),
    }
}

fn strip_newline(line: &str) -> &str {
    line.strip_suffix('\n').unwrap_or(line)
}

/// Two changes closer than twice the context share their context lines, so emitting them
/// apart would print the same lines in both hunks; SHOWN_GAP more are not worth a fold.
fn group(changes: &[imara_diff::Hunk], context: usize) -> Vec<Vec<imara_diff::Hunk>> {
    let gap = (context * 2) as u32 + SHOWN_GAP;
    let mut groups: Vec<Vec<imara_diff::Hunk>> = Vec::new();

    for change in changes {
        match groups.last_mut() {
            Some(last)
                if last.last().is_some_and(|prev| {
                    change.before.start.saturating_sub(prev.before.end) <= gap
                }) =>
            {
                last.push(change.clone());
            }
            _ => groups.push(vec![change.clone()]),
        }
    }
    groups
}

fn build(
    group: &[imara_diff::Hunk],
    old_lines: &[&str],
    new_lines: &[&str],
    options: &DiffOptions,
    open: OpenEnd,
) -> Hunk {
    let context = options.context_lines;
    let first = &group[0];
    let last = &group[group.len() - 1];

    // Around the changes both sides are the same lines, so one offset serves the two.
    let mut old_from = first.before.start.saturating_sub(context);
    if old_from <= SHOWN_GAP {
        old_from = 0;
    }
    let old_len = old_lines.len() as u32;
    let mut old_to = (last.before.end + context).min(old_len);
    if old_len - old_to <= SHOWN_GAP {
        old_to = old_len;
    }
    let new_from = first.after.start - (first.before.start - old_from);
    let new_to = last.after.end + (old_to - last.before.end);

    let inserted: u32 = group.iter().map(|change| change.after.len() as u32).sum();
    let mut rows = Vec::with_capacity((old_to - old_from + inserted) as usize);
    let mut old_at = old_from;
    let mut new_at = new_from;
    // Word diff of each line pair tokenizes into the same two buffers.
    let mut tokens = crate::words::Tokens::default();

    for change in group {
        while old_at < change.before.start {
            rows.push(DiffRow::Context {
                old: old_at + 1,
                new: new_at + 1,
                text: text(old_lines, old_at),
                no_newline: open.old && old_at as usize + 1 == old_lines.len(),
            });
            old_at += 1;
            new_at += 1;
        }
        let deleted: Vec<&str> = change.before.clone().map(|i| line(old_lines, i)).collect();
        let inserted: Vec<&str> = change.after.clone().map(|i| line(new_lines, i)).collect();
        let (old_words, new_words): (Vec<Spans>, Vec<Spans>) =
            if options.word_diff && block_is_comparable(deleted.len(), inserted.len()) {
                deleted
                    .iter()
                    .zip(&inserted)
                    .map(|(old, new)| crate::words::inline_spans_in(old, new, &mut tokens))
                    .unzip()
            } else {
                (Vec::new(), Vec::new())
            };

        let mut old_words = old_words.into_iter();
        for (index, text) in change.before.clone().zip(deleted) {
            rows.push(DiffRow::Delete {
                old: index + 1,
                text: text.to_owned(),
                inline: old_words.next().unwrap_or_default(),
                moved: false,
                move_id: None,
                move_scope: None,
                no_newline: open.old && index as usize + 1 == old_lines.len(),
            });
        }
        let mut new_words = new_words.into_iter();
        for (index, text) in change.after.clone().zip(inserted) {
            rows.push(DiffRow::Insert {
                new: index + 1,
                text: text.to_owned(),
                inline: new_words.next().unwrap_or_default(),
                moved: false,
                move_id: None,
                move_scope: None,
                no_newline: open.new && index as usize + 1 == new_lines.len(),
            });
        }
        old_at = change.before.end;
        new_at = change.after.end;
    }

    while old_at < old_to {
        rows.push(DiffRow::Context {
            old: old_at + 1,
            new: new_at + 1,
            text: text(old_lines, old_at),
            no_newline: open.old && old_at as usize + 1 == old_lines.len(),
        });
        old_at += 1;
        new_at += 1;
    }

    let old_count = old_to - old_from;
    let new_count = new_to - new_from;
    // As git: a side with no lines names the line it follows, zero at the top of the file.
    let old_start = if old_count == 0 {
        old_from
    } else {
        old_from + 1
    };
    let new_start = if new_count == 0 {
        new_from
    } else {
        new_from + 1
    };

    Hunk {
        old_start,
        old_lines: old_count,
        new_start,
        new_lines: new_count,
        header: format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@"),
        rows,
    }
}

fn text(lines: &[&str], index: u32) -> String {
    line(lines, index).to_owned()
}

fn line<'a>(lines: &[&'a str], index: u32) -> &'a str {
    lines
        .get(index as usize)
        .map_or("", |line| strip_newline(line))
}
