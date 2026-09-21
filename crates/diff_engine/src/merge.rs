//! Three-way line merge (doc/08-diff-engine.md §8). Each side is diffed against the base
//! and the two edit scripts are replayed together; where they touch the same base lines
//! and disagree, the region is handed to the user rather than guessed at.

use crate::normalize_line_endings;
use imara_diff::{Diff, InternedInput, sources::lines};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// Nobody touched these lines.
    Unchanged,
    Ours,
    Theirs,
    /// Both sides made the same edit.
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Region {
    Clean {
        lines: Vec<String>,
        /// Anything but `Unchanged` was resolved without asking, and is worth a look
        /// before the merge is committed (doc/08-diff-engine.md §8).
        origin: Origin,
    },
    Conflict {
        base: Vec<String>,
        ours: Vec<String>,
        theirs: Vec<String>,
    },
}

impl Region {
    #[must_use]
    pub fn is_conflict(&self) -> bool {
        matches!(self, Region::Conflict { .. })
    }
}

struct Edit {
    base: Range<usize>,
    lines: Vec<String>,
}

fn split(text: &str) -> Vec<String> {
    let normalised = normalize_line_endings(text);
    if normalised.is_empty() {
        return Vec::new();
    }
    normalised
        .strip_suffix('\n')
        .unwrap_or(&normalised)
        .split('\n')
        .map(str::to_owned)
        .collect()
}

fn edits(base: &str, side: &str) -> Vec<Edit> {
    let input = InternedInput::new(lines(base), lines(side));
    let mut diff = Diff::compute(imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let after: Vec<String> = split(side);
    diff.hunks()
        .map(|hunk| Edit {
            base: hunk.before.start as usize..hunk.before.end as usize,
            lines: hunk
                .after
                .clone()
                .filter_map(|index| after.get(index as usize).cloned())
                .collect(),
        })
        .collect()
}

#[must_use]
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Vec<Region> {
    // Normalised once up front, or a CRLF base makes every line of an LF side a change.
    let base = normalize_line_endings(base);
    let ours = normalize_line_endings(ours);
    let theirs = normalize_line_endings(theirs);

    let base_lines = split(&base);
    let mine = edits(&base, &ours);
    let yours = edits(&base, &theirs);

    let mut regions: Vec<Region> = Vec::new();
    let mut at = 0usize;
    let (mut i, mut j) = (0usize, 0usize);

    while i < mine.len() || j < yours.len() {
        let next = match (mine.get(i), yours.get(j)) {
            (Some(a), Some(b)) => a.base.start.min(b.base.start),
            (Some(a), None) => a.base.start,
            (None, Some(b)) => b.base.start,
            (None, None) => break,
        };
        push_unchanged(&mut regions, &base_lines, at..next);

        // Every edit that touches this stretch of the base, on either side, is one region:
        // two edits that abut each other cannot be applied independently.
        let mut end = next;
        let (from_i, from_j) = (i, j);
        loop {
            let before = (i, j);
            while let Some(edit) = mine.get(i) {
                if edit.base.start > end {
                    break;
                }
                end = end.max(edit.base.end);
                i += 1;
            }
            while let Some(edit) = yours.get(j) {
                if edit.base.start > end {
                    break;
                }
                end = end.max(edit.base.end);
                j += 1;
            }
            if (i, j) == before {
                break;
            }
        }

        let span = next..end;
        let ours_lines = apply(&base_lines, &mine[from_i..i], span.clone());
        let theirs_lines = apply(&base_lines, &yours[from_j..j], span.clone());
        let base_slice: Vec<String> = base_lines[span.clone()].to_vec();

        regions.push(resolve(
            base_slice,
            ours_lines,
            theirs_lines,
            i > from_i,
            j > from_j,
        ));
        at = end;
    }

    push_unchanged(&mut regions, &base_lines, at..base_lines.len());
    if regions.is_empty() {
        regions.push(Region::Clean {
            lines: Vec::new(),
            origin: Origin::Unchanged,
        });
    }
    regions
}

fn resolve(
    base: Vec<String>,
    ours: Vec<String>,
    theirs: Vec<String>,
    ours_changed: bool,
    theirs_changed: bool,
) -> Region {
    if ours == theirs {
        return Region::Clean {
            lines: ours,
            origin: if ours_changed && theirs_changed {
                Origin::Both
            } else {
                Origin::Unchanged
            },
        };
    }
    if !ours_changed || ours == base {
        return Region::Clean {
            lines: theirs,
            origin: Origin::Theirs,
        };
    }
    if !theirs_changed || theirs == base {
        return Region::Clean {
            lines: ours,
            origin: Origin::Ours,
        };
    }
    Region::Conflict { base, ours, theirs }
}

/// The base stretch with one side's edits applied, which is that side's take on it.
fn apply(base: &[String], edits: &[Edit], span: Range<usize>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut at = span.start;
    for edit in edits {
        out.extend_from_slice(&base[at..edit.base.start.max(at)]);
        out.extend(edit.lines.iter().cloned());
        at = edit.base.end.max(at);
    }
    if at < span.end {
        out.extend_from_slice(&base[at..span.end]);
    }
    out
}

fn push_unchanged(regions: &mut Vec<Region>, base: &[String], span: Range<usize>) {
    if span.start >= span.end {
        return;
    }
    regions.push(Region::Clean {
        lines: base[span].to_vec(),
        origin: Origin::Unchanged,
    });
}
