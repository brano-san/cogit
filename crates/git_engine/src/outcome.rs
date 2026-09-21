//! Classifying a finished git command: how bad it was, what to say about it in one
//! line, and what to call it in a window title.
//!
//! The label is read back off the command line that actually ran rather than taken on
//! trust from the caller. SmartGit shows `Command Delete Branch failed!` above the
//! output of a push, because there the two travel separately; here they cannot disagree
//! because there is only one of them (doc/12-risks.md, R-87).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Success,
    Warning,
    Failure,
}

/// Progress is not a warning; git says both on `stderr` and only one is worth a notice.
#[must_use]
pub fn severity_of(exit_code: Option<i32>, stderr: &str) -> Severity {
    if exit_code != Some(0) {
        return Severity::Failure;
    }
    let spoken = stderr
        .lines()
        .any(|line| line.starts_with("warning:") || line.starts_with("hint:"));
    if spoken {
        Severity::Warning
    } else {
        Severity::Success
    }
}

const MARKERS: &[&str] = &["error:", "fatal:", "failed to", "rejected", "! [rejected]"];
const MAX_CHARS: usize = 160;
const MAX_LINES: usize = 2;

/// One or two lines over the output, never instead of it: the window shows both.
#[must_use]
pub fn summarise(stderr: &str, stdout: &str) -> String {
    let source = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };

    let lines: Vec<&str> = source
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return String::new();
    }

    let marked: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            MARKERS.iter().any(|marker| lower.contains(marker))
        })
        .collect();

    // The last line that says what went wrong, or failing that the last line at all.
    // MAX_LINES is the bound the window relies on, not the usual answer.
    let chosen = if marked.is_empty() { &lines } else { &marked };
    let tail = &chosen[chosen.len().saturating_sub(1)..];

    tail.iter()
        .take(MAX_LINES)
        .map(|line| clip(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip(line: &str) -> String {
    if line.chars().count() <= MAX_CHARS {
        return line.to_owned();
    }
    let mut out: String = line.chars().take(MAX_CHARS - 1).collect();
    out.push('…');
    out
}

/// What to call the command in a title, read off the command line itself.
#[must_use]
pub fn operation_label(command_line: &str) -> String {
    let mut words = command_line.split_whitespace().peekable();
    if words.peek() == Some(&"git") {
        words.next();
    }

    // `-c key=value` and friends come before the subcommand.
    let mut rest: Vec<&str> = Vec::new();
    let mut subcommand: Option<&str> = None;
    while let Some(word) = words.next() {
        if subcommand.is_none() {
            if word == "-c" || word == "--git-dir" || word == "--work-tree" {
                words.next();
                continue;
            }
            if word.starts_with('-') {
                continue;
            }
            subcommand = Some(word);
            continue;
        }
        rest.push(word);
    }

    let Some(name) = subcommand else {
        return "Git".to_owned();
    };
    let flags: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|w| w.starts_with('-'))
        .collect();

    match name {
        "fetch" if flags.contains(&"--all") => "Fetch All".to_owned(),
        "pull" if flags.contains(&"--rebase") => "Pull (rebase)".to_owned(),
        "branch" if flags.iter().any(|f| *f == "-d" || *f == "-D") => "Delete Branch".to_owned(),
        "branch" if flags.iter().any(|f| *f == "-m" || *f == "-M") => "Rename Branch".to_owned(),
        "tag" if flags.contains(&"-d") => "Delete Tag".to_owned(),
        "push" if flags.iter().any(|f| f.starts_with("--force")) => "Force Push".to_owned(),
        _ => title_case(name),
    }
}

/// `cherry-pick` reads as `Cherry-pick`, not `Cherry-Pick`: it is one word to a user.
fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Git".to_owned(),
    }
}
