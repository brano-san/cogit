use crate::{DiffRow, Hunk};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatchRequest {
    pub path: String,
    pub hunks: Vec<Hunk>,
    pub selected_deletes: Vec<u32>,
    pub selected_inserts: Vec<u32>,
}

/// How the patch goes on, and which sides of the diff exist as files.
///
/// A selection is cut from the diff on screen. Applied forward (Stage) the patch must match
/// the old side, so an unselected deletion stays as context and an unselected insertion is
/// left out. Applied in reverse (Unstage, Discard) it must match the new side, where it is
/// the other way round.
#[derive(Debug, Clone, Copy)]
pub struct PatchShape {
    pub reverse: bool,
    pub old_exists: bool,
    pub new_exists: bool,
}

/// The two files the diff was cut from, as they are now and as git holds them — the working
/// file cleaned as `git add` would store it. The patch is written in their own lines and
/// endings: with whitespace ignored a context line differs between the sides, only the side
/// the patch goes onto has the text `git apply` will look for, and a mixed file has no one
/// ending to put back.
#[derive(Debug, Clone, Copy)]
pub struct PatchSides<'a> {
    pub old: &'a [u8],
    pub new: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchError {
    NothingSelected,
    /// The file is not the one the diff was cut from any more.
    Stale,
    /// A line the patch carries is not UTF-8.
    NotUtf8,
    /// A lone CR ends a line for the diff but not for git, so the lines do not line up.
    BareCarriageReturn,
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NothingSelected => "nothing selected",
            Self::Stale => "the file changed since its diff was shown: look at it again first",
            Self::NotUtf8 => "a selected line is not valid UTF-8: stage it whole instead",
            Self::BareCarriageReturn => {
                "a line ends in a lone CR, which git does not count as a line break: stage it whole instead"
            }
        })
    }
}

impl std::error::Error for PatchError {}

/// Whether a line of the diff holds bytes that were not UTF-8. The viewer shows each one
/// as a stand-in character (see `text::decode`); a patch would write the stand-in into the
/// file instead of the byte, so such a file is staged whole, never line by line.
#[must_use]
pub fn carries_undecoded_bytes(request: &PatchRequest) -> bool {
    request.hunks.iter().flat_map(|hunk| &hunk.rows).any(|row| {
        let text = match row {
            DiffRow::Context { text, .. }
            | DiffRow::Delete { text, .. }
            | DiffRow::Insert { text, .. } => text,
            DiffRow::Collapsed { .. } => return false,
        };
        text.chars().any(|c| ('\u{F780}'..='\u{F7FF}').contains(&c))
    })
}

/// One line of a side as the diff numbered it; `ending` is empty on a last line without one.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    body: &'a [u8],
    ending: &'a [u8],
}

/// Split where the diff split, after CRLF and LF; a lone CR, which the diff splits at too
/// and git does not, is refused.
fn split_lines(bytes: &[u8]) -> Result<Vec<Line<'_>>, PatchError> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (at, &byte) in bytes.iter().enumerate() {
        if byte == b'\r' && bytes.get(at + 1) != Some(&b'\n') {
            return Err(PatchError::BareCarriageReturn);
        }
        if byte == b'\n' {
            let cr = at > start && bytes[at - 1] == b'\r';
            let end = if cr { at - 1 } else { at };
            lines.push(Line {
                body: &bytes[start..end],
                ending: &bytes[end..=at],
            });
            start = at + 1;
        }
    }
    if start < bytes.len() {
        lines.push(Line {
            body: &bytes[start..],
            ending: &[],
        });
    }
    Ok(lines)
}

/// Line `number` (from 1) of a side: its text and its ending.
fn line<'a>(side: &[Line<'a>], number: u32) -> Result<(&'a str, &'a str), PatchError> {
    let line = number
        .checked_sub(1)
        .and_then(|index| side.get(index as usize))
        .ok_or(PatchError::Stale)?;
    let text = std::str::from_utf8(line.body).map_err(|_| PatchError::NotUtf8)?;
    let ending = std::str::from_utf8(line.ending).map_err(|_| PatchError::NotUtf8)?;
    Ok((text, ending))
}

/// Only the selected lines. The envelope stays LF; each content line keeps the ending it
/// has on its own side, or `git apply` rewrites every line (INV-08).
pub fn build_patch(
    request: &PatchRequest,
    shape: PatchShape,
    sides: PatchSides<'_>,
) -> Result<String, PatchError> {
    let deletes: HashSet<u32> = request.selected_deletes.iter().copied().collect();
    let inserts: HashSet<u32> = request.selected_inserts.iter().copied().collect();
    if deletes.is_empty() && inserts.is_empty() {
        return Err(PatchError::NothingSelected);
    }
    let old_lines = split_lines(sides.old)?;
    let new_lines = split_lines(sides.new)?;

    let mut body = String::new();
    let (mut pre_total, mut post_total) = (0_u32, 0_u32);
    // New minus old line numbers past every hunk so far, carried or not: it places a hunk
    // on the side it has no lines on.
    let mut offset = 0_i64;
    // Post minus pre over the hunks this patch carries so far: how far the side being
    // written has moved by the time the next hunk lands.
    let mut applied = 0_i64;

    for hunk in &request.hunks {
        let (old_from, new_from) = if hunk.old_lines > 0 {
            let old = i64::from(hunk.old_start) - 1;
            (old, old + offset)
        } else {
            let new = i64::from(hunk.new_start.max(1)) - 1;
            (new - offset, new)
        };
        offset += i64::from(hunk.new_lines) - i64::from(hunk.old_lines);

        let mut lines: Vec<(char, &str, &str)> = Vec::new();
        let (mut pre, mut post) = (0_u32, 0_u32);
        let mut changed = false;
        for row in &hunk.rows {
            let (marker, (text, ending)) = match row {
                // The side the patch goes onto: its text is what `git apply` matches.
                DiffRow::Context { old, new, .. } => (
                    ' ',
                    if shape.reverse {
                        line(&new_lines, *new)?
                    } else {
                        line(&old_lines, *old)?
                    },
                ),
                DiffRow::Delete { old, .. } => {
                    if deletes.contains(old) {
                        changed = true;
                        ('-', line(&old_lines, *old)?)
                    } else if shape.reverse {
                        continue;
                    } else {
                        (' ', line(&old_lines, *old)?)
                    }
                }
                DiffRow::Insert { new, .. } => {
                    if inserts.contains(new) {
                        changed = true;
                        ('+', line(&new_lines, *new)?)
                    } else if shape.reverse {
                        (' ', line(&new_lines, *new)?)
                    } else {
                        continue;
                    }
                }
                DiffRow::Collapsed { .. } => continue,
            };
            if marker != '+' {
                pre += 1;
            }
            if marker != '-' {
                post += 1;
            }
            lines.push((marker, text, ending));
        }
        if !changed {
            continue;
        }
        let lines = close_open_ends(lines);

        let (pre_from, post_from) = if shape.reverse {
            (new_from - applied, new_from)
        } else {
            (old_from, old_from + applied)
        };
        // A side with no lines names the line it comes after, as git writes it.
        let pre_start = pre_from + i64::from(pre > 0);
        let post_start = post_from + i64::from(post > 0);
        body.push_str(&format!("@@ -{pre_start},{pre} +{post_start},{post} @@\n"));
        for (marker, text, ending) in lines {
            body.push(marker);
            body.push_str(text);
            if ending.is_empty() {
                body.push_str("\n\\ No newline at end of file\n");
            } else {
                body.push_str(ending);
            }
        }
        applied += i64::from(post) - i64::from(pre);
        pre_total += pre;
        post_total += post;
    }

    if body.is_empty() {
        return Err(PatchError::NothingSelected);
    }
    // `/dev/null` means the file is not there on that side: not before the patch when it
    // does not exist yet, not after it when every one of its lines goes.
    let path = &request.path;
    let from = if !shape.old_exists && pre_total == 0 {
        "/dev/null".to_owned()
    } else {
        format!("a/{path}")
    };
    let to = if !shape.new_exists && post_total == 0 {
        "/dev/null".to_owned()
    } else {
        format!("b/{path}")
    };
    Ok(format!("--- {from}\n+++ {to}\n{body}"))
}

/// A line without a final newline can only be the last one of its side. Cutting a selection
/// can put lines after it — the insertions after an unselected last line, the context after
/// a selected one in reverse — and git then glues the next line onto it. There the line
/// gains its newline, as git-gui writes it: a context line splits into `-` and `+`. The
/// ending is the one of the line before it.
fn close_open_ends<'a>(lines: Vec<(char, &'a str, &'a str)>) -> Vec<(char, &'a str, &'a str)> {
    let mut out: Vec<(char, &str, &str)> = Vec::with_capacity(lines.len() + 1);
    for (at, &(marker, text, ending)) in lines.iter().enumerate() {
        if !ending.is_empty() {
            out.push((marker, text, ending));
            continue;
        }
        let rest = &lines[at + 1..];
        let old_goes_on = rest.iter().any(|(m, ..)| *m != '+');
        let new_goes_on = rest.iter().any(|(m, ..)| *m != '-');
        let closed = lines[..at]
            .iter()
            .rev()
            .map(|(_, _, ending)| *ending)
            .find(|ending| !ending.is_empty())
            .unwrap_or("\n");
        let end = |goes_on: bool| if goes_on { closed } else { "" };
        match marker {
            ' ' if old_goes_on || new_goes_on => {
                out.push(('-', text, end(old_goes_on)));
                out.push(('+', text, end(new_goes_on)));
            }
            '-' => out.push((marker, text, end(old_goes_on))),
            '+' => out.push((marker, text, end(new_goes_on))),
            _ => out.push((marker, text, "")),
        }
    }
    out
}
