use crate::{DiffRow, Hunk, LineEnding};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatchRequest {
    pub path: String,
    pub hunks: Vec<Hunk>,
    pub selected_deletes: Vec<u32>,
    pub selected_inserts: Vec<u32>,
    pub line_ending: LineEnding,
    pub no_trailing_newline: bool,
}

/// Only the selected lines. The envelope stays LF; content lines keep the file's own
/// ending, or `git apply` rewrites every line (INV-08).
#[must_use]
pub fn build_patch(request: &PatchRequest) -> Option<String> {
    let deletes: HashSet<u32> = request.selected_deletes.iter().copied().collect();
    let inserts: HashSet<u32> = request.selected_inserts.iter().copied().collect();
    if deletes.is_empty() && inserts.is_empty() {
        return None;
    }

    let mut body = String::new();
    let mut any = false;

    for hunk in &request.hunks {
        let mut lines: Vec<(char, &str)> = Vec::new();
        let mut old_count = 0_u32;
        let mut new_count = 0_u32;
        let mut changed = false;

        for row in &hunk.rows {
            match row {
                DiffRow::Context { text, .. } => {
                    lines.push((' ', text));
                    old_count += 1;
                    new_count += 1;
                }
                DiffRow::Delete { old, text, .. } => {
                    if deletes.contains(old) {
                        lines.push(('-', text));
                        old_count += 1;
                        changed = true;
                    } else {
                        lines.push((' ', text));
                        old_count += 1;
                        new_count += 1;
                    }
                }
                DiffRow::Insert { new, text, .. } => {
                    if inserts.contains(new) {
                        lines.push(('+', text));
                        new_count += 1;
                        changed = true;
                    }
                }
                DiffRow::Collapsed { .. } => {}
            }
        }

        if !changed {
            continue;
        }
        any = true;

        let old_start = if old_count == 0 {
            0
        } else {
            hunk.old_start.max(1)
        };
        let new_start = if new_count == 0 {
            0
        } else {
            hunk.new_start.max(1)
        };
        body.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        for (marker, text) in lines {
            body.push(marker);
            body.push_str(text);
            body.push_str(request.line_ending.as_str());
        }
    }

    if !any {
        return None;
    }
    if request.no_trailing_newline {
        body.push_str("\\ No newline at end of file\n");
    }

    let (from, to) = envelope(request, &deletes, &inserts);
    Some(format!("--- {from}\n+++ {to}\n{body}"))
}

fn envelope(
    request: &PatchRequest,
    deletes: &HashSet<u32>,
    inserts: &HashSet<u32>,
) -> (String, String) {
    let path = &request.path;
    let creating = request.hunks.iter().all(|h| h.old_lines == 0);
    let deleting = request.hunks.iter().all(|h| h.new_lines == 0);

    if creating && !inserts.is_empty() {
        ("/dev/null".to_owned(), format!("b/{path}"))
    } else if deleting && !deletes.is_empty() {
        (format!("a/{path}"), "/dev/null".to_owned())
    } else {
        (format!("a/{path}"), format!("b/{path}"))
    }
}

impl LineEnding {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
            _ => "\n",
        }
    }
}
