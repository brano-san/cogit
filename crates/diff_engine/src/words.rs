use similar::{ChangeTag, TextDiff};

const MAX_BLOCK_RATIO: usize = 100;

pub type Spans = Vec<(u32, u32)>;

fn is_operator_char(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~' | '?' | ':'
    )
}

fn tokenize(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            while let Some(&(_, next)) = chars.peek() {
                if next.is_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
        } else if ch.is_alphanumeric() || ch == '_' {
            while let Some(&(_, next)) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
        } else if ch == '.' {
            while let Some(&(_, next)) = chars.peek() {
                if next == '.' {
                    chars.next();
                } else {
                    break;
                }
            }
        } else if is_operator_char(ch) {
            while let Some(&(_, next)) = chars.peek() {
                if is_operator_char(next) {
                    chars.next();
                } else {
                    break;
                }
            }
        }
        let end = chars.peek().map_or(text.len(), |&(idx, _)| idx);
        tokens.push(&text[start..end]);
    }

    tokens
}

/// Changed words on each side, in **UTF-16 units**: the consumer is JavaScript, where a
/// string index is a UTF-16 unit (doc/12-risks.md, R-10).
#[must_use]
pub fn inline_spans(old: &str, new: &str) -> (Spans, Spans) {
    let old_tokens = tokenize(old);
    let new_tokens = tokenize(new);
    let diff = TextDiff::from_slices(&old_tokens, &new_tokens);

    let mut old_spans = Spans::new();
    let mut new_spans = Spans::new();
    let mut old_at = 0_u32;
    let mut new_at = 0_u32;
    let mut old_soft = false;
    let mut new_soft = false;

    for change in diff.iter_all_changes() {
        let width = utf16_len(change.value());
        match change.tag() {
            ChangeTag::Equal => {
                let blank = change.value().trim().is_empty();
                old_soft &= blank;
                new_soft &= blank;
                old_at += width;
                new_at += width;
            }
            ChangeTag::Delete => {
                push(&mut old_spans, old_at, old_at + width, old_soft);
                old_at += width;
                old_soft = true;
            }
            ChangeTag::Insert => {
                push(&mut new_spans, new_at, new_at + width, new_soft);
                new_at += width;
                new_soft = true;
            }
        }
    }

    (old_spans, new_spans)
}

#[must_use]
pub fn block_is_comparable(deleted: usize, inserted: usize) -> bool {
    let (low, high) = (deleted.min(inserted), deleted.max(inserted));
    low > 0 && high <= low * MAX_BLOCK_RATIO
}

fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

/// Words separated only by whitespace read as one highlight, not as stripes.
fn push(spans: &mut Spans, from: u32, to: u32, soft: bool) {
    match spans.last_mut() {
        Some(last) if soft && last.1 <= from => last.1 = to,
        _ => spans.push((from, to)),
    }
}
