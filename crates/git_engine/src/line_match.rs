//! Line diffs and fuzzy block search for Investigate's origin candidates (R-281).

use gix::diff::blob::{Algorithm, Diff, InternedInput};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineHunk {
    pub before: Range<u32>,
    pub after: Range<u32>,
}

pub(crate) fn line_hunks(before: &[String], after: &[String]) -> Vec<LineHunk> {
    let mut input: InternedInput<&str> = InternedInput::default();
    input.update_before(before.iter().map(String::as_str));
    input.update_after(after.iter().map(String::as_str));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    diff.hunks()
        .map(|hunk| LineHunk {
            before: hunk.before,
            after: hunk.after,
        })
        .collect()
}

/// Which new lines of a hunk replaced an old one: all when as many go in as out (as in
/// SmartGit's Blame), else those resembling an old line, else the first ones.
pub(crate) fn replaced_lines(before: &[String], after: &[String]) -> Vec<bool> {
    if before.len() == after.len() {
        return vec![true; after.len()];
    }
    let lower = |lines: &[String]| -> Vec<String> {
        lines.iter().map(|line| key(line).to_lowercase()).collect()
    };
    let (old, new) = (lower(before), lower(after));
    let mut flags = vec![false; after.len()];
    let mut next = 0;
    for (k, line) in new.iter().enumerate() {
        if let Some(found) = (next..old.len()).find(|&i| similarity(&old[i], line) >= SIMILAR) {
            flags[k] = true;
            next = found + 1;
        }
    }
    if !flags.contains(&true) {
        for flag in flags.iter_mut().take(before.len()) {
            *flag = true;
        }
    }
    flags
}

pub(crate) fn normalise(line: &str, ignore_whitespace: bool) -> String {
    if ignore_whitespace {
        line.chars().filter(|c| !c.is_whitespace()).collect()
    } else {
        line.to_owned()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockMatch {
    /// Source lines covered, 0-based, half-open.
    pub source: Range<usize>,
    /// Per block line: the source line it pairs with and how alike the two are.
    pub pairs: Vec<Option<(usize, f32)>>,
    /// 0..=1; bigger blocks and closer text score higher.
    pub score: f32,
    /// Share of the block found, weighted by how telling each line is.
    pub coverage: f32,
    /// Weight of the lines found verbatim; zero means only look-alikes paired.
    pub anchored: f32,
}

/// A line pairs with another this alike or more even when the text differs.
const SIMILAR: f32 = 0.5;
const MIN_SCORE: f32 = 0.25;
const MAX_DIAGONALS: usize = 16;
/// A line repeated more often than this in the source says nothing about position.
const MAX_OCCURRENCES: usize = 24;

/// Every place `block` plausibly came from in `source`, best first. `exclude` hides
/// matches that mostly overlap a range already accounted for.
pub(crate) fn find_block(
    block: &[String],
    source: &[String],
    exclude: Option<Range<usize>>,
) -> Vec<BlockMatch> {
    let block_keys: Vec<String> = block.iter().map(|line| key(line)).collect();
    let source_keys: Vec<String> = source.iter().map(|line| key(line)).collect();

    let mut index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (at, line) in source_keys.iter().enumerate() {
        if weight(line) >= 0.5 {
            index.entry(line.as_str()).or_default().push(at);
        }
    }

    let mut votes: HashMap<isize, f32> = HashMap::new();
    for (i, line) in block_keys.iter().enumerate() {
        let Some(found) = index.get(line.as_str()) else {
            continue;
        };
        if found.len() > MAX_OCCURRENCES {
            continue;
        }
        for &j in found {
            *votes.entry(signed(j) - signed(i)).or_default() += weight(line);
        }
    }
    let mut diagonals: Vec<(isize, f32)> = votes.into_iter().collect();
    diagonals.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

    let slack = (block.len() / 2).max(3);
    let mut found: Vec<BlockMatch> = Vec::new();
    for (diagonal, _) in diagonals.into_iter().take(MAX_DIAGONALS) {
        let start = usize::try_from(diagonal - signed(slack)).unwrap_or(0);
        let end = usize::try_from(diagonal + signed(block.len() + slack))
            .unwrap_or(0)
            .min(source.len());
        if start >= end {
            continue;
        }
        let Some(candidate) = align_keys(&block_keys, &source_keys, start..end) else {
            continue;
        };
        if candidate.score < MIN_SCORE
            || candidate.anchored <= 0.0
            || exclude
                .as_ref()
                .is_some_and(|range| mostly_overlaps(&candidate.source, range))
        {
            continue;
        }
        match found
            .iter_mut()
            .find(|seen| mostly_overlaps(&seen.source, &candidate.source))
        {
            Some(seen) if seen.score < candidate.score => *seen = candidate,
            Some(_) => {}
            None => found.push(candidate),
        }
    }
    found.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.source.start.cmp(&b.source.start))
    });
    found
}

/// Scores `block` against one fixed window of `source`: the lines replaced in place.
pub(crate) fn align(
    block: &[String],
    source: &[String],
    window: Range<usize>,
) -> Option<BlockMatch> {
    let block_keys: Vec<String> = block.iter().map(|line| key(line)).collect();
    let source_keys: Vec<String> = source.iter().map(|line| key(line)).collect();
    align_keys(&block_keys, &source_keys, window)
}

fn align_keys(block: &[String], source: &[String], window: Range<usize>) -> Option<BlockMatch> {
    let slice = source.get(window.clone())?;
    let mut pairs: Vec<Option<(usize, f32)>> = vec![None; block.len()];

    let mut cursor = (0, 0);
    for hunk in line_hunks(block, slice) {
        let (before, after) = (to_range(&hunk.before), to_range(&hunk.after));
        pair_unchanged(
            &mut pairs,
            cursor,
            (before.start, after.start),
            window.start,
        );
        for (i, j) in before.clone().zip(after.clone()) {
            let alike = similarity(&block[i], &slice[j]);
            if alike >= SIMILAR {
                pairs[i] = Some((window.start + j, alike));
            }
        }
        cursor = (before.end, after.end);
    }
    pair_unchanged(&mut pairs, cursor, (block.len(), slice.len()), window.start);

    let total: f32 = block.iter().map(|line| weight(line)).sum();
    let matched: f32 = block
        .iter()
        .zip(&pairs)
        .filter_map(|(line, pair)| pair.map(|(_, alike)| weight(line) * alike))
        .sum();
    let anchored: f32 = block
        .iter()
        .zip(&pairs)
        .filter(|(line, pair)| weight(line) >= 0.5 && pair.is_some_and(|(_, alike)| alike >= 1.0))
        .map(|(line, _)| weight(line))
        .sum();
    let first = pairs.iter().flatten().map(|(j, _)| *j).min()?;
    let last = pairs.iter().flatten().map(|(j, _)| *j).max()?;
    if total <= 0.0 {
        return None;
    }

    let paired = pairs.iter().flatten().count() as f32;
    let compact = paired / (last - first + 1) as f32;
    let size = (0.5 + anchored / 6.0).min(1.0);
    let coverage = matched / total;
    Some(BlockMatch {
        source: first..last + 1,
        pairs,
        score: coverage * (0.75 + 0.25 * compact) * size,
        coverage,
        anchored,
    })
}

fn pair_unchanged(
    pairs: &mut [Option<(usize, f32)>],
    from: (usize, usize),
    to: (usize, usize),
    offset: usize,
) {
    for (i, j) in (from.0..to.0).zip(from.1..to.1) {
        if let Some(pair) = pairs.get_mut(i) {
            *pair = Some((offset + j, 1.0));
        }
    }
}

fn key(line: &str) -> String {
    normalise(line, true)
}

/// How much a line says about where it came from: `}` says nothing, a statement a lot.
fn weight(key: &str) -> f32 {
    let letters = key.chars().filter(|c| c.is_alphanumeric()).count();
    (letters as f32 / 8.0).clamp(0.1, 1.0)
}

/// Dice coefficient over character pairs.
pub(crate) fn similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let mut left = bigrams(a);
    let mut right = bigrams(b);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    left.sort_unstable();
    right.sort_unstable();
    let (mut i, mut j, mut shared) = (0, 0, 0_usize);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
        }
    }
    (2 * shared) as f32 / (left.len() + right.len()) as f32
}

fn bigrams(text: &str) -> Vec<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

fn mostly_overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    let shared = a.end.min(b.end).saturating_sub(a.start.max(b.start));
    let smaller = a.len().min(b.len()).max(1);
    shared * 2 > smaller
}

fn signed(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

fn to_range(range: &Range<u32>) -> Range<usize> {
    range.start as usize..range.end as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    const BLOCK: &str = "local function rotate(cat, angle)\n\
                         local radians = math.rad(angle)\n\
                         cat.x = cat.x * math.cos(radians)\n\
                         cat.y = cat.y * math.sin(radians)\n\
                         return cat\n\
                         end";

    fn source_with(block: &str) -> Vec<String> {
        let mut text =
            String::from("print('header one')\nlocal unrelated = compute_the_answer()\n");
        text.push_str(block);
        text.push_str("\nprint('footer that closes the file')\n");
        lines(&text)
    }

    #[test]
    fn an_identical_block_is_found_where_it_sits() {
        let found = find_block(&lines(BLOCK), &source_with(BLOCK), None);

        assert_eq!(found[0].source, 2..8);
        assert!(found[0].score > 0.95, "{found:?}");
    }

    #[test]
    fn a_block_with_an_edited_line_and_new_indentation_is_still_found() {
        let edited = BLOCK
            .replace("math.sin(radians)", "math.sin(radians) + offset")
            .replace("local radians", "    local radians");

        let found = find_block(&lines(BLOCK), &source_with(&edited), None);

        assert_eq!(found[0].source, 2..8);
        assert!(found[0].score > 0.75, "{found:?}");
        assert!(
            found[0].pairs[3].is_some(),
            "the edited line pairs with its old self"
        );
    }

    #[test]
    fn a_closing_brace_alone_matches_nothing() {
        let found = find_block(&lines("}"), &lines("fn a() {\n}\nfn b() {\n}"), None);

        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn the_whole_block_outranks_a_place_holding_one_of_its_lines() {
        let mut source = lines("local radians = math.rad(angle)\nprint('elsewhere')");
        source.extend(source_with(BLOCK));

        let found = find_block(&lines(BLOCK), &source, None);

        assert_eq!(found[0].source, 4..10);
        assert!(found.len() <= 2);
        assert!(
            found
                .iter()
                .skip(1)
                .all(|other| other.score < found[0].score)
        );
    }

    #[test]
    fn an_excluded_range_hides_the_match_inside_it() {
        let found = find_block(&lines(BLOCK), &source_with(BLOCK), Some(2..8));

        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn similar_lines_score_between_nothing_and_everything() {
        assert!((similarity("abc", "abc") - 1.0).abs() < f32::EPSILON);
        assert!(similarity("cat.y = cat.y * sin(r)", "cat.y = cat.y * sin(r) + o") > 0.8);
        assert!(similarity("alpha", "zzzzz") < 0.1);
    }

    #[test]
    fn as_many_lines_in_as_out_are_all_replacements() {
        assert_eq!(replaced_lines(&lines("a\nb"), &lines("x\ny")), [true, true]);
    }

    #[test]
    fn a_new_line_resembling_an_old_one_replaced_it_and_the_rest_were_inserted() {
        let flags = replaced_lines(
            &lines("let total = 1;"),
            &lines("// sum them up\nlet total = 2;\nlet more = 3;"),
        );

        assert_eq!(flags, [false, true, false]);
    }

    #[test]
    fn with_no_resemblance_the_first_lines_take_the_old_places() {
        assert_eq!(
            replaced_lines(&lines("abc"), &lines("xyz\nqrs")),
            [true, false]
        );
    }

    #[test]
    fn hunks_report_what_changed_on_each_side() {
        let hunks = line_hunks(&lines("a\nb\nc"), &lines("a\nB\nc\nd"));

        assert_eq!(
            hunks,
            [
                LineHunk {
                    before: 1..2,
                    after: 1..2
                },
                LineHunk {
                    before: 3..3,
                    after: 3..4
                }
            ]
        );
    }
}
