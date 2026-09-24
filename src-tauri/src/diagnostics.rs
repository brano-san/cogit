//! One block of text that answers "what were you running, and what did it say".
//!
//! `Help ▸ Copy Diagnostics` puts it on the clipboard so a report can be pasted into a
//! ticket without asking the user to find a folder (doc/14-profiling.md).

use std::path::Path;

/// How much of the log travels with a report. Enough to hold the failure and what led to
/// it; short enough to paste into a chat window.
pub const TAIL_LINES: usize = 200;

/// The last `lines` lines, oldest first.
///
/// Reads the whole file rather than seeking backwards: the log is capped at 10 MB by
/// rotation, and a diagnostics copy happens once, by hand.
#[must_use]
pub fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let from = all.len().saturating_sub(lines);
    all[from..].join("\n")
}

fn read_tail(log_path: &Path, lines: usize) -> String {
    match std::fs::read_to_string(log_path) {
        Ok(text) => tail(&text, lines),
        Err(err) => format!("(cannot read {}: {err})", log_path.display()),
    }
}

/// The whole report. Paths are printed even when reading them failed: half the questions
/// are answered by the path alone.
#[must_use]
pub fn report(log_path: &Path, config_dir: &Path, webview2: Option<&str>) -> String {
    // The path kept at start-up is the first part; a long session has gone on past it.
    let log_path = &app_state::logging::latest_part(log_path);
    let mut out = String::new();

    out.push_str(&format!("Cogit {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!(
        "WebView2: {}\n",
        webview2.unwrap_or("not installed or too old to report")
    ));
    out.push_str(&format!("OS: {}\n", std::env::consts::OS));
    out.push_str(&format!("Build: {}\n", build_kind()));
    out.push_str(&format!("Log: {}\n", log_path.display()));
    out.push_str(&format!("Config: {}\n", config_dir.display()));
    out.push_str(&format!("\n--- last {TAIL_LINES} log lines ---\n"));
    out.push_str(&read_tail(log_path, TAIL_LINES));
    out.push('\n');

    out
}

fn build_kind() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_log_travels_whole() {
        assert_eq!(tail("one\ntwo", 200), "one\ntwo");
    }

    #[test]
    fn a_long_log_keeps_its_end() {
        let text = (1..=500)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let kept = tail(&text, 200);
        assert!(kept.starts_with("301\n"));
        assert!(kept.ends_with("\n500"));
        assert_eq!(kept.lines().count(), 200);
    }

    #[test]
    fn an_empty_log_is_not_an_error() {
        assert_eq!(tail("", 200), "");
    }

    #[test]
    fn a_missing_file_says_so_instead_of_failing() {
        let text = read_tail(Path::new("no-such-file.log"), 10);
        assert!(text.contains("cannot read"));
    }

    #[test]
    fn the_report_leads_with_what_was_running() {
        let text = report(
            Path::new("C:/logs/cogit.log"),
            Path::new("C:/cfg"),
            Some("120.0.1"),
        );
        assert!(text.starts_with("Cogit "));
        assert!(text.contains("WebView2: 120.0.1"));
        assert!(text.contains("C:/logs/cogit.log"));
    }

    #[test]
    fn a_missing_runtime_is_reported_rather_than_left_blank() {
        let text = report(Path::new("C:/logs/cogit.log"), Path::new("C:/cfg"), None);
        assert!(text.contains("not installed or too old"));
    }
}
