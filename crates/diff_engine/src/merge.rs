//! Three-way line merge (doc/08-diff-engine.md §8). Each side is diffed against the base
//! and the two edit scripts are replayed together; where they touch the same base lines
//! and disagree, the region is handed to the user rather than guessed at.

use crate::normalize_line_endings;
use imara_diff::{Diff, InternedInput, sources::lines};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// Nobody touched these lines.
    Unchanged,
    Ours,
    Theirs,
    /// Both sides made the same edit.
    Both,
    /// Settled by the parser: the two sides touched different nodes of the tree, so the
    /// order is not a guess. Always worth a look before the merge is committed.
    Syntactic,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
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

/// One side's edit of the base: the base lines it replaces and the side's lines there.
pub(crate) struct Edit<'a> {
    pub(crate) base: Range<usize>,
    pub(crate) lines: &'a [&'a str],
}

/// Lines of a text already normalised to LF, without their newlines.
fn split(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

fn edits<'a>(base: &str, side: &str, after: &'a [&'a str]) -> Vec<Edit<'a>> {
    let input = InternedInput::new(lines(base), lines(side));
    let mut diff = Diff::compute(imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let within = |at: u32| (at as usize).min(after.len());
    diff.hunks()
        .map(|hunk| Edit {
            base: hunk.before.start as usize..hunk.before.end as usize,
            lines: &after
                [within(hunk.after.start)..within(hunk.after.end).max(within(hunk.after.start))],
        })
        .collect()
}

/// One stretch of the file with what every side has there, for the Conflict Solver: it
/// aligns the panes by stretch, and "Base Changes" needs what each side did to the base.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SolverRegion {
    pub kind: SolverKind,
    pub base: Vec<String>,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    /// What the merge puts there on its own; for a conflict, the base stretch (undecided).
    pub result: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SolverKind {
    Equal,
    /// Only our side changed the stretch; the result has our lines.
    Ours,
    Theirs,
    Both,
    Syntactic,
    Conflict,
}

/// One stretch of the merge, borrowed from the sides; owned lines are made only for the
/// answer that is sent.
struct Stretch<'a> {
    base: &'a [&'a str],
    ours: Cow<'a, [&'a str]>,
    theirs: Cow<'a, [&'a str]>,
    /// `None` for a conflict.
    merged: Option<(Merged<'a>, Origin)>,
}

/// What the merge put in a clean stretch.
enum Merged<'a> {
    Ours,
    Theirs,
    Lines(Vec<&'a str>),
}

impl Stretch<'_> {
    fn merged(&self) -> Option<(&[&str], Origin)> {
        let (merged, origin) = self.merged.as_ref()?;
        let lines = match merged {
            Merged::Ours => &self.ours,
            Merged::Theirs => &self.theirs,
            Merged::Lines(lines) => lines.as_slice(),
        };
        Some((lines, *origin))
    }
}

fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|&line| line.to_owned()).collect()
}

#[must_use]
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Vec<Region> {
    merge3_with_syntax(base, ours, theirs, None)
}

#[must_use]
pub fn merge3_sides(
    base: &str,
    ours: &str,
    theirs: &str,
    language: Option<&str>,
) -> Vec<SolverRegion> {
    merge_all(base, ours, theirs, language, |stretch| {
        let (kind, result) = match stretch.merged() {
            None => (SolverKind::Conflict, stretch.base),
            Some((lines, origin)) => (
                match origin {
                    Origin::Unchanged => SolverKind::Equal,
                    Origin::Ours => SolverKind::Ours,
                    Origin::Theirs => SolverKind::Theirs,
                    Origin::Both => SolverKind::Both,
                    Origin::Syntactic => SolverKind::Syntactic,
                },
                lines,
            ),
        };
        SolverRegion {
            kind,
            base: owned(stretch.base),
            ours: owned(&stretch.ours),
            theirs: owned(&stretch.theirs),
            result: owned(result),
        }
    })
}

/// The same merge, with one more rule: where the two sides changed different top-level
/// nodes of the parsed base, their edits are combined instead of handed over. Only with
/// full certainty — an unparsable file or an edit that spans two nodes stays a conflict
/// (doc/08-diff-engine.md §8).
#[must_use]
pub fn merge3_with_syntax(
    base: &str,
    ours: &str,
    theirs: &str,
    language: Option<&str>,
) -> Vec<Region> {
    merge_all(base, ours, theirs, language, |stretch| {
        match stretch.merged() {
            Some((lines, origin)) => Region::Clean {
                lines: owned(lines),
                origin,
            },
            None => Region::Conflict {
                base: owned(stretch.base),
                ours: owned(&stretch.ours),
                theirs: owned(&stretch.theirs),
            },
        }
    })
}

fn merge_all<T>(
    base: &str,
    ours: &str,
    theirs: &str,
    language: Option<&str>,
    mut emit: impl FnMut(Stretch<'_>) -> T,
) -> Vec<T> {
    // Normalised once up front, or a CRLF base makes every line of an LF side a change.
    let base = normalize_line_endings(base);
    let ours = normalize_line_endings(ours);
    let theirs = normalize_line_endings(theirs);

    let base_lines = split(&base);
    let ours_lines = split(&ours);
    let theirs_lines = split(&theirs);
    let mine = edits(&base, &ours, &ours_lines);
    let yours = edits(&base, &theirs, &theirs_lines);
    let nodes = language.and_then(|name| crate::syntax::top_level_nodes(&base, name));

    let mut regions: Vec<T> = Vec::new();
    let mut at = 0usize;
    let (mut i, mut j) = (0usize, 0usize);

    while i < mine.len() || j < yours.len() {
        let next = match (mine.get(i), yours.get(j)) {
            (Some(a), Some(b)) => a.base.start.min(b.base.start),
            (Some(a), None) => a.base.start,
            (None, Some(b)) => b.base.start,
            (None, None) => break,
        };
        push_unchanged(&mut regions, &mut emit, &base_lines, at..next);

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
        let ours = apply(&base_lines, &mine[from_i..i], span.clone());
        let theirs = apply(&base_lines, &yours[from_j..j], span.clone());
        let base = &base_lines[span.clone()];

        let merged = resolve(base, &ours, &theirs, i > from_i, j > from_j).or_else(|| {
            crate::syntax::settle(
                nodes.as_deref()?,
                &base_lines,
                span.clone(),
                &mine[from_i..i],
                &yours[from_j..j],
            )
            .map(|lines| (Merged::Lines(lines), Origin::Syntactic))
        });
        regions.push(emit(Stretch {
            base,
            ours: Cow::Owned(ours),
            theirs: Cow::Owned(theirs),
            merged,
        }));
        at = end;
    }

    push_unchanged(&mut regions, &mut emit, &base_lines, at..base_lines.len());
    if regions.is_empty() {
        regions.push(emit(Stretch {
            base: &[],
            ours: Cow::Borrowed(&[]),
            theirs: Cow::Borrowed(&[]),
            merged: Some((Merged::Ours, Origin::Unchanged)),
        }));
    }
    regions
}

/// Which side's lines a stretch settles on by itself; `None` is a conflict.
fn resolve<'a>(
    base: &[&str],
    ours: &[&str],
    theirs: &[&str],
    ours_changed: bool,
    theirs_changed: bool,
) -> Option<(Merged<'a>, Origin)> {
    if ours == theirs {
        let origin = if ours_changed && theirs_changed {
            Origin::Both
        } else {
            Origin::Unchanged
        };
        return Some((Merged::Ours, origin));
    }
    if !ours_changed || ours == base {
        return Some((Merged::Theirs, Origin::Theirs));
    }
    if !theirs_changed || theirs == base {
        return Some((Merged::Ours, Origin::Ours));
    }
    None
}

/// The base stretch with one side's edits applied, which is that side's take on it.
pub(crate) fn apply<'a, 'e>(
    base: &[&'a str],
    edits: impl IntoIterator<Item = &'e Edit<'a>>,
    span: Range<usize>,
) -> Vec<&'a str>
where
    'a: 'e,
{
    let mut out: Vec<&str> = Vec::new();
    let mut at = span.start;
    for edit in edits {
        out.extend_from_slice(&base[at..edit.base.start.max(at)]);
        out.extend_from_slice(edit.lines);
        at = edit.base.end.max(at);
    }
    if at < span.end {
        out.extend_from_slice(&base[at..span.end]);
    }
    out
}

fn push_unchanged<'a, T>(
    regions: &mut Vec<T>,
    emit: &mut impl FnMut(Stretch<'_>) -> T,
    base: &'a [&'a str],
    span: Range<usize>,
) {
    if span.start >= span.end {
        return;
    }
    let lines = &base[span];
    regions.push(emit(Stretch {
        base: lines,
        ours: Cow::Borrowed(lines),
        theirs: Cow::Borrowed(lines),
        merged: Some((Merged::Ours, Origin::Unchanged)),
    }));
}
