use crate::blame_origins::{PreviousFile, UNCOMMITTED};
use crate::line_match::{BlockMatch, LineHunk, align, find_block, line_hunks};
use crate::{FileStatus, GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::Range;

/// A block of lines and the commit that introduced it, as blame reported them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginQuery {
    /// The commit that wrote the block; forty zeros for the working tree.
    pub commit: String,
    pub path: String,
    /// The block, 1-based and inclusive, in that commit's version of `path`.
    pub from: u32,
    pub to: u32,
    /// The line the user picked, inside the block.
    pub line: u32,
    pub previous: Option<PreviousFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OriginKind {
    /// Nothing like it existed before: the lines first appeared at this position.
    Appeared,
    /// The lines replaced others at the same position.
    Modified,
    /// Similar lines were removed elsewhere by the same commit.
    Moved,
    /// Similar lines existed elsewhere and stayed there.
    Copied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Likelihood {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LineMatch {
    Same,
    Changed,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginText {
    pub line: u32,
    pub text: String,
    pub status: LineMatch,
}

/// Where Go Deeper continues: the source's version, file and the picked line in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeeperTarget {
    pub rev: String,
    pub path: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginCandidate {
    pub kind: OriginKind,
    /// The commit whose tree holds the source lines.
    pub rev: String,
    pub path: String,
    pub from: u32,
    pub to: u32,
    /// Percent.
    pub score: u32,
    pub likelihood: Likelihood,
    pub deeper: Option<DeeperTarget>,
    /// Per block line: how it compares with the source.
    pub block: Vec<LineMatch>,
    pub source: Vec<OriginText>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginReport {
    /// In-place first, the rest by score.
    pub candidates: Vec<OriginCandidate>,
    /// Index of the highest-scoring candidate.
    pub best: u32,
}

const MAX_FILES: usize = 300;
const MAX_SOURCE_LINES: usize = 50_000;
const MAX_CANDIDATES: usize = 10;
const PER_FILE: usize = 3;
const MAX_COPY_SOURCES: usize = 200;
/// A candidate this good ends the search before the sweep over untouched files.
const STRONG: f32 = 0.75;

struct Found {
    kind: OriginKind,
    path: String,
    matched: BlockMatch,
    source: Vec<String>,
}

impl RepoHandle {
    /// Where the block stood before `query.commit`, best guesses first; `None` once
    /// `cancelled` says the answer is no longer wanted (R-281).
    pub fn origin_candidates(
        &self,
        query: &OriginQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<OriginReport>> {
        if cancelled() {
            return Ok(None);
        }
        let after = self
            .text_lines(&query.commit, &query.path)?
            .ok_or_else(|| {
                GitError::InvalidState(format!("{} is not in {}", query.path, query.commit))
            })?;
        let span = block_span(query, after.len())?;
        let block = &after[span.clone()];

        let previous = match &query.previous {
            Some(previous) => Some(previous.clone()),
            None => self.implied_previous(query),
        };
        let parent = previous
            .as_ref()
            .map(|previous| previous.oid.clone())
            .or_else(|| self.parent_of(&query.commit));
        let Some(parent) = parent else {
            return Ok(Some(report(query, block, Vec::new(), None)));
        };

        let mut found = Vec::new();
        let mut appeared_at = None;
        if let Some(previous) = &previous
            && let Some(before) = self.text_lines(&previous.oid, &previous.path)?
        {
            let hunks = line_hunks(&before, &after);
            let touching: Vec<&LineHunk> = hunks
                .iter()
                .filter(|hunk| overlaps(&to_range(&hunk.after), &span))
                .collect();
            let removed = touching
                .iter()
                .filter(|hunk| !hunk.before.is_empty())
                .map(|hunk| to_range(&hunk.before))
                .reduce(|a, b| a.start.min(b.start)..a.end.max(b.end));

            if let Some(removed) = removed.clone() {
                let matched = in_place(block, &before, removed);
                found.push(Found {
                    kind: OriginKind::Modified,
                    path: previous.path.clone(),
                    matched,
                    source: before.clone(),
                });
            } else {
                let at = touching
                    .first()
                    .map_or(0, |hunk| hunk.before.start as usize);
                appeared_at = Some(DeeperTarget {
                    rev: parent.clone(),
                    path: previous.path.clone(),
                    line: line_number(at.min(before.len().saturating_sub(1))),
                });
            }
            for matched in find_block(block, &before, removed) {
                let kind = if removed_by(&hunks, &matched.source) {
                    OriginKind::Moved
                } else {
                    OriginKind::Copied
                };
                found.push(Found {
                    kind,
                    path: previous.path.clone(),
                    matched,
                    source: before.clone(),
                });
            }
        }

        let mut searched: HashSet<String> = [query.path.clone()].into_iter().collect();
        searched.extend(previous.as_ref().map(|p| p.path.clone()));
        let changed = self.changed_files(&query.commit)?;
        let strong = |found: &[Found]| found.iter().any(|f| f.matched.score >= STRONG);
        for (old, new, deleted) in changed {
            if cancelled() {
                return Ok(None);
            }
            if searched.contains(&old) || searched.contains(&new) {
                continue;
            }
            searched.insert(old.clone());
            let source = match self.text_lines(&parent, &old) {
                Ok(Some(source)) if readable(&source) => source,
                Ok(_) => continue,
                Err(err) => {
                    tracing::debug!(error = ?err, path = %old, context = "origin search skipped a file");
                    continue;
                }
            };
            let matches = find_block(block, &source, None);
            if matches.is_empty() {
                continue;
            }
            let hunks = if deleted {
                None
            } else {
                self.text_lines(&query.commit, &new)
                    .ok()
                    .flatten()
                    .map(|now| line_hunks(&source, &now))
            };
            for matched in matches.into_iter().take(PER_FILE) {
                let kind = match &hunks {
                    Some(hunks) if !removed_by(hunks, &matched.source) => OriginKind::Copied,
                    _ => OriginKind::Moved,
                };
                found.push(Found {
                    kind,
                    path: old.clone(),
                    matched,
                    source: source.clone(),
                });
            }
        }

        if !strong(&found) {
            for path in self.copy_sources(&parent, &query.path, &searched) {
                if cancelled() {
                    return Ok(None);
                }
                let Ok(Some(source)) = self.text_lines(&parent, &path) else {
                    continue;
                };
                if !readable(&source) {
                    continue;
                }
                for matched in find_block(block, &source, None).into_iter().take(PER_FILE) {
                    found.push(Found {
                        kind: OriginKind::Copied,
                        path: path.clone(),
                        matched,
                        source: source.clone(),
                    });
                }
            }
        }
        if cancelled() {
            return Ok(None);
        }

        let offset = (query.line.saturating_sub(query.from)) as usize;
        let candidates = found
            .into_iter()
            .map(|found| candidate(&found, &parent, offset))
            .collect();
        Ok(Some(report(query, block, candidates, appeared_at)))
    }

    /// The same path one commit back, when the commit's first parent has it.
    fn implied_previous(&self, query: &OriginQuery) -> Option<PreviousFile> {
        let parent = self.parent_of(&query.commit)?;
        match self.blob_at(&parent, &query.path) {
            Ok(Some(_)) => Some(PreviousFile {
                oid: parent,
                path: query.path.clone(),
            }),
            _ => None,
        }
    }

    /// The first parent, or HEAD for the working tree.
    fn parent_of(&self, commit: &str) -> Option<String> {
        let rev = if commit == UNCOMMITTED {
            "HEAD".to_owned()
        } else {
            format!("{commit}^")
        };
        self.repo
            .rev_parse_single(rev.as_str())
            .ok()
            .map(|id| id.detach().to_string())
    }

    /// Files the commit did not touch that a block may have been copied from: those with
    /// the same extension, nearest folders first, a bounded number (R-281).
    fn copy_sources(&self, rev: &str, path: &str, searched: &HashSet<String>) -> Vec<String> {
        let Ok(tree) = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| err.to_string())
            .and_then(|id| id.object().map_err(|err| err.to_string()))
            .and_then(|object| object.peel_to_tree().map_err(|err| err.to_string()))
        else {
            return Vec::new();
        };
        let mut recorder = gix::traverse::tree::Recorder::default();
        if tree.traverse().breadthfirst(&mut recorder).is_err() {
            return Vec::new();
        }
        let extension = extension_of(path);
        let mut paths: Vec<String> = recorder
            .records
            .into_iter()
            .filter(|entry| entry.mode.is_blob())
            .map(|entry| entry.filepath.to_string())
            .filter(|candidate| {
                extension_of(candidate) == extension && !searched.contains(candidate)
            })
            .collect();
        paths.sort_by_key(|candidate| {
            (
                std::cmp::Reverse(shared_folders(candidate, path)),
                candidate.clone(),
            )
        });
        paths.truncate(MAX_COPY_SOURCES);
        paths
    }

    /// `(path before, path after, deleted)` for every file the commit changed.
    fn changed_files(&self, commit: &str) -> Result<Vec<(String, String, bool)>> {
        let entries = if commit == UNCOMMITTED {
            let files = self.worktree_files()?;
            let mut all = files.staged;
            all.extend(files.unstaged);
            all
        } else {
            self.commit_files(commit)?
        };
        let mut changed: Vec<(String, String, bool)> = entries
            .into_iter()
            .filter_map(|entry| match entry.status {
                FileStatus::Modified => Some((entry.path.clone(), entry.path, false)),
                FileStatus::Deleted => Some((entry.path.clone(), entry.path, true)),
                FileStatus::Renamed => {
                    Some((entry.old_path.unwrap_or_default(), entry.path, false))
                }
                FileStatus::Copied => entry.old_path.map(|old| (old.clone(), old, false)),
                _ => None,
            })
            .collect();
        changed.sort();
        changed.dedup();
        changed.truncate(MAX_FILES);
        Ok(changed)
    }
}

fn block_span(query: &OriginQuery, lines: usize) -> Result<Range<usize>> {
    let (from, to) = (query.from as usize, query.to as usize);
    if from == 0 || to < from || to > lines || query.line < query.from || query.line > query.to {
        return Err(GitError::InvalidState(format!(
            "lines {from}..{to} (picked {}) are not inside {} of {lines} lines",
            query.line, query.path
        )));
    }
    Ok(from - 1..to)
}

/// The lines a commit replaced in place always count as a candidate: the position alone
/// says a lot, and how alike the old text is decides how much more.
fn in_place(block: &[String], before: &[String], removed: Range<usize>) -> BlockMatch {
    let matched = align(block, before, removed.clone());
    let coverage = matched.as_ref().map_or(0.0, |found| found.coverage);
    let mut found = matched.unwrap_or(BlockMatch {
        source: removed.clone(),
        pairs: vec![None; block.len()],
        score: 0.0,
        coverage: 0.0,
        anchored: 0.0,
    });
    found.source = removed;
    found.score = 0.4 + 0.6 * coverage;
    found
}

fn candidate(found: &Found, parent: &str, offset: usize) -> OriginCandidate {
    let matched = &found.matched;
    let status = |alike: f32| {
        if alike >= 1.0 {
            LineMatch::Same
        } else {
            LineMatch::Changed
        }
    };
    let block = matched
        .pairs
        .iter()
        .map(|pair| pair.map_or(LineMatch::Missing, |(_, alike)| status(alike)))
        .collect();
    let source = matched
        .source
        .clone()
        .map(|at| OriginText {
            line: line_number(at),
            text: found.source.get(at).cloned().unwrap_or_default(),
            status: matched
                .pairs
                .iter()
                .flatten()
                .find(|(j, _)| *j == at)
                .map_or(LineMatch::Missing, |(_, alike)| status(*alike)),
        })
        .collect();
    let score = percent(matched.score);
    OriginCandidate {
        kind: found.kind,
        rev: parent.to_owned(),
        path: found.path.clone(),
        from: line_number(matched.source.start),
        to: line_number(matched.source.end.saturating_sub(1)),
        score,
        likelihood: likelihood(score),
        deeper: Some(DeeperTarget {
            rev: parent.to_owned(),
            path: found.path.clone(),
            line: line_number(map_line(matched, offset)),
        }),
        block,
        source,
    }
}

/// Assembles the list: an in-place candidate first, the rest by score. With nothing in
/// place, "appeared here" competes with whatever was found elsewhere.
fn report(
    query: &OriginQuery,
    block: &[String],
    mut candidates: Vec<OriginCandidate>,
    appeared_at: Option<DeeperTarget>,
) -> OriginReport {
    candidates.sort_by(|a, b| {
        let in_place = |c: &OriginCandidate| c.kind != OriginKind::Modified;
        in_place(a).cmp(&in_place(b)).then(b.score.cmp(&a.score))
    });
    candidates.truncate(MAX_CANDIDATES);

    if !candidates.iter().any(|c| c.kind == OriginKind::Modified) {
        let rival = candidates.iter().map(|c| c.score).max().unwrap_or(0);
        let score = 100_u32.saturating_sub(rival).max(10);
        candidates.insert(
            0,
            OriginCandidate {
                kind: OriginKind::Appeared,
                rev: query.commit.clone(),
                path: query.path.clone(),
                from: query.from,
                to: query.to,
                score,
                likelihood: likelihood(score),
                deeper: appeared_at,
                block: vec![LineMatch::Same; block.len()],
                source: block
                    .iter()
                    .zip(query.from..)
                    .map(|(text, line)| OriginText {
                        line,
                        text: text.clone(),
                        status: LineMatch::Same,
                    })
                    .collect(),
            },
        );
    }

    let best = candidates
        .iter()
        .enumerate()
        .fold((0, 0), |(at, top), (index, c)| {
            if c.score > top {
                (index, c.score)
            } else {
                (at, top)
            }
        })
        .0;
    OriginReport {
        candidates,
        best: u32::try_from(best).unwrap_or(0),
    }
}

/// The source line the picked block line pairs with, or the nearest pairing's neighbour.
fn map_line(matched: &BlockMatch, offset: usize) -> usize {
    if let Some(Some((at, _))) = matched.pairs.get(offset) {
        return *at;
    }
    let above = matched.pairs[..offset.min(matched.pairs.len())]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(k, pair)| pair.map(|(at, _)| at + (offset - k)));
    let below = matched
        .pairs
        .iter()
        .enumerate()
        .skip(offset + 1)
        .find_map(|(k, pair)| pair.map(|(at, _)| at.saturating_sub(k - offset)));
    above.or(below).unwrap_or(matched.source.start).clamp(
        matched.source.start,
        matched
            .source
            .end
            .saturating_sub(1)
            .max(matched.source.start),
    )
}

fn removed_by(hunks: &[LineHunk], source: &Range<usize>) -> bool {
    let removed = source
        .clone()
        .filter(|at| hunks.iter().any(|hunk| to_range(&hunk.before).contains(at)))
        .count();
    removed * 2 > source.len()
}

fn extension_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map_or("", |(_, extension)| extension)
}

fn shared_folders(a: &str, b: &str) -> usize {
    a.split('/')
        .zip(b.split('/'))
        .take_while(|(x, y)| x == y)
        .count()
}

fn readable(lines: &[String]) -> bool {
    lines.len() <= MAX_SOURCE_LINES && !lines.iter().any(|line| line.contains('\0'))
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

fn to_range(range: &Range<u32>) -> Range<usize> {
    range.start as usize..range.end as usize
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

fn percent(score: f32) -> u32 {
    (score.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn likelihood(score: u32) -> Likelihood {
    match score {
        75.. => Likelihood::High,
        45.. => Likelihood::Medium,
        _ => Likelihood::Low,
    }
}
