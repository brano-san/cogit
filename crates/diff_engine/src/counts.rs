use imara_diff::{Algorithm, Diff, InternedInput, sources::byte_lines};
use std::borrow::Cow;

#[must_use]
pub fn line_counts(old: &[u8], new: &[u8]) -> Option<(u32, u32)> {
    let size = old.len().max(new.len()) as u64;
    if size >= crate::MAX_TEXT_BYTES || crate::binary::cause(old, new).is_some() {
        return None;
    }
    let (old, new) = (unix_endings(old), unix_endings(new));
    let input = InternedInput::new(byte_lines(&old), byte_lines(&new));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    Some((diff.count_additions(), diff.count_removals()))
}

fn unix_endings(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.windows(2).any(|pair| pair == b"\r\n") {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes.iter().peekable();
    while let Some(&byte) = rest.next() {
        if byte == b'\r' && rest.peek() == Some(&&b'\n') {
            continue;
        }
        out.push(byte);
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_what_git_numstat_counts() {
        assert_eq!(line_counts(b"a\nb\nc\n", b"a\nB\nc\nd\n"), Some((2, 1)));
    }

    #[test]
    fn a_new_file_is_all_additions() {
        assert_eq!(line_counts(b"", b"one\ntwo\n"), Some((2, 0)));
    }

    #[test]
    fn line_endings_alone_change_nothing() {
        assert_eq!(line_counts(b"a\r\nb\r\n", b"a\nb\n"), Some((0, 0)));
    }

    #[test]
    fn binary_sides_have_no_line_counts() {
        assert_eq!(line_counts(b"\0\x01\x02", b"text\n"), None);
    }
}
