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

/// What a renderer may be handed. A test log of a hundred thousand lines is a real thing
/// to hit — the whole point of the window is that a hook printed it — and handing it to
/// the DOM whole is how the webview runs out of memory. The full text stays in the log
/// file; the window shows the beginning, the end, and how much it is not showing.
pub const MAX_LINES: usize = 20_000;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const HEAD_LINES: usize = 2_000;
const TAIL_LINES: usize = 5_000;

/// Both limits, in that order. Text under either one comes back byte for byte.
#[must_use]
pub fn trim(text: &str) -> String {
    match by_lines(text) {
        Some(short) => by_bytes(&short).unwrap_or(short),
        None => by_bytes(text).unwrap_or_else(|| text.to_owned()),
    }
}

/// `None` when the text is already short enough to pass through untouched.
fn by_lines(text: &str) -> Option<String> {
    let total = text.split_inclusive('\n').count();
    if total <= MAX_LINES {
        return None;
    }

    let mut lines = text.split_inclusive('\n');
    let mut out = String::new();
    for line in lines.by_ref().take(HEAD_LINES) {
        out.push_str(line);
    }

    let rest: Vec<&str> = lines.collect();
    let tail = rest.len().saturating_sub(TAIL_LINES);
    out.push_str(&format!("… {tail} lines omitted, see log …\n"));
    for line in &rest[tail..] {
        out.push_str(line);
    }
    Some(out)
}

/// The backstop for output that is few lines and enormous anyway — a minified bundle in
/// a diff, a base64 blob a hook echoed.
fn by_bytes(text: &str) -> Option<String> {
    if text.len() <= MAX_BYTES {
        return None;
    }
    let half = MAX_BYTES / 2;
    let mut head = half;
    while head > 0 && !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - half;
    while tail < text.len() && !text.is_char_boundary(tail) {
        tail += 1;
    }
    let omitted = tail - head;
    Some(format!(
        "{}\n… {omitted} bytes omitted, see log …\n{}",
        &text[..head],
        &text[tail..]
    ))
}

/// Everything a stream goes through before anyone can see it: cleaned up, then cut down.
#[must_use]
pub fn shown(text: &str) -> String {
    trim(&normalise(text))
}
