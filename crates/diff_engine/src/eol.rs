//! Without normalization a Windows checkout shows every line of every file as changed (INV-08).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineEnding {
    Lf,
    Crlf,
    Cr,
    Mixed,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EolInfo {
    pub old: LineEnding,
    pub new: LineEnding,
    pub normalized: bool,
}

#[must_use]
pub fn detect_line_ending(text: &str) -> LineEnding {
    let bytes = text.as_bytes();
    let mut crlf = 0_usize;
    let mut lf = 0_usize;
    let mut cr = 0_usize;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                crlf += 1;
                i += 2;
                continue;
            }
            b'\r' => cr += 1,
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }

    match (crlf > 0, lf > 0, cr > 0) {
        (false, false, false) => LineEnding::None,
        (true, false, false) => LineEnding::Crlf,
        (false, true, false) => LineEnding::Lf,
        (false, false, true) => LineEnding::Cr,
        _ => LineEnding::Mixed,
    }
}

/// CRLF to LF. A lone CR stays inside its line: git ends lines at LF only, and the line
/// numbers must be the ones blame and `log -L` use.
#[must_use]
pub fn normalize_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("\r\n") {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_each_pure_style() {
        assert_eq!(detect_line_ending("a\nb\n"), LineEnding::Lf);
        assert_eq!(detect_line_ending("a\r\nb\r\n"), LineEnding::Crlf);
        assert_eq!(detect_line_ending("a\rb\r"), LineEnding::Cr);
        assert_eq!(detect_line_ending("single line"), LineEnding::None);
    }

    #[test]
    fn detects_mixed_endings() {
        assert_eq!(detect_line_ending("a\r\nb\nc\r\n"), LineEnding::Mixed);
    }

    #[test]
    fn a_lone_cr_before_text_is_not_counted_as_crlf() {
        assert_eq!(detect_line_ending("a\rb"), LineEnding::Cr);
    }

    #[test]
    fn normalization_turns_crlf_into_lf_and_keeps_a_lone_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\nd"), "a\nb\rc\nd");
    }

    #[test]
    fn lf_text_is_returned_without_allocating() {
        let input = "a\nb\nc";
        assert!(matches!(
            normalize_line_endings(input),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn crlf_and_lf_files_are_identical_after_normalization() {
        // The whole point of INV-08: these two must not produce a diff.
        let crlf = "fn main() {\r\n    println!();\r\n}\r\n";
        let lf = "fn main() {\n    println!();\n}\n";
        assert_eq!(normalize_line_endings(crlf), normalize_line_endings(lf));
    }
}
