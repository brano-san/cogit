use crate::{
    Algorithm, DiffOptions, DiffRow, EolInfo, FileDiff, Hunk, Whitespace, block_is_comparable,
    detect_line_ending, inline_spans, normalize_line_endings,
};
use imara_diff::{Diff, InternedInput, Interner, Token, sources::lines};
use std::borrow::Cow;

/// Above this a file is shown as a summary: rendering it would cost more than it tells.
pub const MAX_TEXT_BYTES: u64 = 1024 * 1024;

/// Git's own rule: a NUL byte anywhere in the first 8000 bytes means binary.
const BINARY_SNIFF_BYTES: usize = 8000;

#[must_use]
pub fn diff_bytes(old: &[u8], new: &[u8], options: &DiffOptions) -> FileDiff {
    let old_size = old.len() as u64;
    let new_size = new.len() as u64;

    if old_size > MAX_TEXT_BYTES || new_size > MAX_TEXT_BYTES {
        return FileDiff::TooLarge {
            size: old_size.max(new_size),
        };
    }
    if old == new {
        return FileDiff::Unchanged;
    }
    if let Some(mime) = crate::image_mime(old).or_else(|| crate::image_mime(new)) {
        return FileDiff::Image {
            old_size,
            new_size,
            mime: mime.to_owned(),
        };
    }
    if is_binary(old) || is_binary(new) {
        return FileDiff::Binary { old_size, new_size };
    }

    let old_text = String::from_utf8_lossy(old);
    let new_text = String::from_utf8_lossy(new);
    let lossy = matches!(old_text, Cow::Owned(_)) || matches!(new_text, Cow::Owned(_));

    let mut diff = diff_text(&old_text, &new_text, options);
    if let FileDiff::Text { lossy_encoding, .. } = &mut diff {
        *lossy_encoding = lossy;
    }
    diff
}

#[must_use]
pub fn diff_text(old: &str, new: &str, options: &DiffOptions) -> FileDiff {
    if old == new {
        return FileDiff::Unchanged;
    }

    let old_eol = detect_line_ending(old);
    let new_eol = detect_line_ending(new);
    let old_text = normalize_line_endings(old);
    let new_text = normalize_line_endings(new);

    if old_text == new_text {
        return FileDiff::EolOnly {
            from: old_eol,
            to: new_eol,
        };
    }

    let old_lines: Vec<&str> = lines(&old_text).collect();
    let new_lines: Vec<&str> = lines(&new_text).collect();

    let open = OpenEnd {
        old: lacks_final_newline(&old_text),
        new: lacks_final_newline(&new_text),
    };

    let old_keys: Vec<Cow<'_, str>> = open.keys(&old_lines, true, options);
    let new_keys: Vec<Cow<'_, str>> = open.keys(&new_lines, false, options);

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

/// A line the file ends on without a newline is not the same line as the same text with
/// one — git prints `\ No newline at end of file` for exactly that difference. Marking the
/// key is what makes the diff see it; the sentinel holds a NUL, which cannot appear in
/// text that got this far, because `diff_bytes` calls such content binary.
const OPEN_END: &str = "\u{0}no-final-newline";

impl OpenEnd {
    fn side(self, old: bool) -> bool {
        if old { self.old } else { self.new }
    }

    fn keys<'a>(self, lines: &[&'a str], old: bool, options: &DiffOptions) -> Vec<Cow<'a, str>> {
        let last = lines.len().saturating_sub(1);
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let body = key(line, options);
                if self.side(old) && index == last {
                    Cow::Owned(format!("{body}{OPEN_END}"))
                } else {
                    body
                }
            })
            .collect()
    }
}

fn lacks_final_newline(text: &str) -> bool {
    !text.is_empty() && !text.ends_with('\n')
}

fn key<'a>(line: &'a str, options: &DiffOptions) -> Cow<'a, str> {
    let body = strip_newline(line);
    match options.ignore_whitespace {
        Whitespace::None => Cow::Borrowed(body),
        Whitespace::Trailing => Cow::Borrowed(body.trim_end()),
        Whitespace::All => Cow::Owned(body.split_whitespace().collect::<Vec<_>>().join(" ")),
    }
}

fn strip_newline(line: &str) -> &str {
    line.strip_suffix('\n').unwrap_or(line)
}

fn is_binary(data: &[u8]) -> bool {
    data.iter().take(BINARY_SNIFF_BYTES).any(|&b| b == 0)
}

/// Two changes closer than twice the context share their context lines, so emitting them
/// apart would print the same lines in both hunks.
fn group(changes: &[imara_diff::Hunk], context: usize) -> Vec<Vec<imara_diff::Hunk>> {
    let gap = (context * 2) as u32;
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

    let old_from = first.before.start.saturating_sub(context);
    let old_to = (last.before.end + context).min(old_lines.len() as u32);
    let new_from = first.after.start.saturating_sub(context);
    let new_to = (last.after.end + context).min(new_lines.len() as u32);

    let mut rows = Vec::new();
    let mut old_at = old_from;
    let mut new_at = new_from;

    for change in group {
        while old_at < change.before.start {
            rows.push(DiffRow::Context {
                old: old_at + 1,
                new: new_at + 1,
                text: text(old_lines, old_at),
            });
            old_at += 1;
            new_at += 1;
        }
        let deleted: Vec<String> = change.before.clone().map(|i| text(old_lines, i)).collect();
        let inserted: Vec<String> = change.after.clone().map(|i| text(new_lines, i)).collect();
        let words = if options.word_diff && block_is_comparable(deleted.len(), inserted.len()) {
            deleted
                .iter()
                .zip(inserted.iter())
                .map(|(old, new)| inline_spans(old, new))
                .collect()
        } else {
            Vec::new()
        };

        for (offset, index) in change.before.clone().enumerate() {
            rows.push(DiffRow::Delete {
                old: index + 1,
                text: deleted[offset].clone(),
                inline: words
                    .get(offset)
                    .map(|(old, _)| old.clone())
                    .unwrap_or_default(),
                moved: false,
                move_id: None,
                move_scope: None,
                no_newline: open.old && index as usize + 1 == old_lines.len(),
            });
        }
        for (offset, index) in change.after.clone().enumerate() {
            rows.push(DiffRow::Insert {
                new: index + 1,
                text: inserted[offset].clone(),
                inline: words
                    .get(offset)
                    .map(|(_, new)| new.clone())
                    .unwrap_or_default(),
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
        });
        old_at += 1;
        new_at += 1;
    }

    let old_count = old_to - old_from;
    let new_count = new_to - new_from;
    let old_start = if old_count == 0 { 0 } else { old_from + 1 };
    let new_start = if new_count == 0 { 0 } else { new_from + 1 };

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
    lines
        .get(index as usize)
        .map(|line| strip_newline(line).to_owned())
        .unwrap_or_default()
}
