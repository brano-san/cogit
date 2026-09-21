//! What git and its hooks actually print, made fit to show and to paste into a bug
//! report: no colour codes, no hundred redraws of one progress line, no credentials.
//!
//! Everything else is left byte for byte. The output of a compiler or a test runner is
//! only readable with its own indentation and blank lines (INV-05).

/// `ESC [ … final-byte`, plus the two-character sequences git occasionally emits.
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            // CSI: parameters and intermediates, then one byte in @..~ ends it.
            Some('[') => {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            // OSC: runs until BEL or ESC \.
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A progress line is rewritten in place with `\r`; only its last state is worth keeping.
#[must_use]
pub fn collapse_progress(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }

    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, ending) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        // `\r\n` is a line ending, not a redraw: keep the body whole.
        let body = body.strip_suffix('\r').unwrap_or(body);
        let last = body.rsplit('\r').next().unwrap_or(body);
        out.push_str(last);
        out.push_str(ending);
    }
    out
}

const HIDDEN: &str = "***";

/// Credentials that turn up in what git prints back: the remote URL it could not read,
/// a header it echoed, an environment dump from a hook.
#[must_use]
pub fn redact_secrets(text: &str) -> String {
    text.split_inclusive('\n').map(redact_line).collect()
}

fn redact_line(line: &str) -> String {
    let line = redact_urls(line);
    if let Some(cut) = header_value_at(&line) {
        return format!("{}{HIDDEN}{}", &line[..cut.0], &line[cut.1..]);
    }
    redact_assignment(&line)
}

/// `scheme://user:secret@host`, anywhere in the line.
fn redact_urls(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(at) = rest.find("://") {
        let (before, tail) = rest.split_at(at + 3);
        let end = tail
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(tail.len());
        let (authority, after) = tail.split_at(end);

        out.push_str(before);
        match authority.split_once('@').and_then(|(creds, host)| {
            creds
                .split_once(':')
                .map(|(user, _)| format!("{user}:{HIDDEN}@{host}"))
        }) {
            Some(safe) => out.push_str(&safe),
            None => out.push_str(authority),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The byte range of the value after `Authorization:`, if the line carries one.
fn header_value_at(line: &str) -> Option<(usize, usize)> {
    let lower = line.to_ascii_lowercase();
    let at = lower.find("authorization:")?;
    let value = at + "authorization:".len();
    let end = line.len() - line[value..].len() + line[value..].trim_end().len();
    Some((
        value + (line[value..].len() - line[value..].trim_start().len()),
        end,
    ))
}

const SECRET_WORDS: &[&str] = &["token", "password", "passwd", "secret", "apikey", "api_key"];

/// `NAME=value` where the name says the value is a secret.
fn redact_assignment(line: &str) -> String {
    let Some((name, _)) = line.split_once('=') else {
        return line.to_owned();
    };
    let bare = name
        .trim()
        .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '_');
    let lower = bare.to_ascii_lowercase();
    if !SECRET_WORDS.iter().any(|word| lower.contains(word)) {
        return line.to_owned();
    }

    let ending = if line.ends_with('\n') { "\n" } else { "" };
    format!("{name}={HIDDEN}{ending}")
}

/// Colour off, redraws collapsed, credentials hidden. Everything else untouched.
#[must_use]
pub fn normalise(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    redact_secrets(&collapse_progress(&strip_ansi(text)))
}
