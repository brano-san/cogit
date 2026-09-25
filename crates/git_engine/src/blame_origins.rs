use crate::line_match::{line_hunks, normalise, replaced_lines};
use crate::{RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What the commit that introduced a line did to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LineChange {
    /// Nothing stood at this position in the parent.
    Added,
    /// The line replaced a different one at the same position.
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub merge: bool,
    /// A root commit: blame could look no further back.
    pub boundary: bool,
    /// The working tree's own lines, which no commit holds yet.
    pub uncommitted: bool,
}

/// The file a commit's lines were read from before that commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviousFile {
    pub oid: String,
    pub path: String,
}

/// One commit and the file it wrote the lines into; `previous` is the same file one
/// step back, absent when the commit created it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlameSource {
    pub commit: u32,
    pub path: String,
    pub previous: Option<PreviousFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginLine {
    pub line: u32,
    pub text: String,
    /// Index into `BlameReport::sources`.
    pub source: u32,
    /// The line's number in the source commit's version of the source file.
    pub orig_line: u32,
    pub change: LineChange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlameReport {
    pub commits: Vec<BlameCommit>,
    pub sources: Vec<BlameSource>,
    pub lines: Vec<OriginLine>,
}

pub(crate) const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

impl RepoHandle {
    /// `git blame --porcelain -M -C -C`: every line with the commit, file and line it came
    /// from. `rev` of `None` blames the working tree file.
    pub fn blame_origins(
        &self,
        path: &str,
        rev: Option<&str>,
        ignore_whitespace: bool,
    ) -> Result<BlameReport> {
        let mut args = vec![
            "-c",
            "core.quotepath=off",
            "blame",
            "--porcelain",
            "-M",
            "-C",
            "-C",
        ];
        if ignore_whitespace {
            args.push("-w");
        }
        if let Some(rev) = rev {
            args.push(rev);
        }
        args.extend(["--", path]);

        let mut report = parse_porcelain(&self.read_git(&args)?);
        for commit in &mut report.commits {
            commit.merge = !commit.uncommitted && self.parent_count(&commit.oid) > 1;
        }
        self.mark_changes(&mut report, ignore_whitespace);
        Ok(report)
    }

    pub(crate) fn parent_count(&self, oid: &str) -> usize {
        gix::ObjectId::from_hex(oid.as_bytes())
            .ok()
            .and_then(|id| self.repo.find_commit(id).ok())
            .map_or(0, |commit| commit.parent_ids().count())
    }

    /// Lines as text, `\r` dropped; `None` when the file is absent there.
    pub(crate) fn text_lines(&self, rev: &str, path: &str) -> Result<Option<Vec<String>>> {
        let bytes = if rev == UNCOMMITTED {
            self.blob_on_disk(path)?
        } else {
            self.blob_at(rev, path)?
        };
        Ok(bytes.map(|bytes| split_lines(&String::from_utf8_lossy(&bytes))))
    }

    /// Diffs each source against its previous version: a line in a hunk that also
    /// removed lines was modified, one in a pure insertion was added.
    fn mark_changes(&self, report: &mut BlameReport, ignore_whitespace: bool) {
        let mut modified: HashMap<u32, Vec<bool>> = HashMap::new();
        for (index, source) in report.sources.iter().enumerate() {
            let Some(previous) = &source.previous else {
                continue;
            };
            let Some(commit) = report.commits.get(source.commit as usize) else {
                continue;
            };
            let rev = if commit.uncommitted {
                UNCOMMITTED
            } else {
                commit.oid.as_str()
            };
            let (Ok(Some(before)), Ok(Some(after))) = (
                self.text_lines(&previous.oid, &previous.path),
                self.text_lines(rev, &source.path),
            ) else {
                tracing::debug!(path = %source.path, context = "no versions to mark changes from");
                continue;
            };
            let key = |line: &String| normalise(line, ignore_whitespace);
            let before: Vec<String> = before.iter().map(key).collect();
            let after: Vec<String> = after.iter().map(key).collect();
            let mut flags = vec![false; after.len()];
            for hunk in line_hunks(&before, &after) {
                let old = &before[hunk.before.start as usize..hunk.before.end as usize];
                let new = &after[hunk.after.start as usize..hunk.after.end as usize];
                for (at, replaced) in hunk.after.zip(replaced_lines(old, new)) {
                    if let Some(flag) = flags.get_mut(at as usize) {
                        *flag = replaced;
                    }
                }
            }
            modified.insert(u32::try_from(index).unwrap_or(u32::MAX), flags);
        }

        for line in &mut report.lines {
            let flagged = modified
                .get(&line.source)
                .and_then(|flags| flags.get(line.orig_line.saturating_sub(1) as usize))
                .copied()
                .unwrap_or(false);
            if flagged {
                line.change = LineChange::Modified;
            }
        }
    }
}

pub(crate) fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect()
}

#[derive(Default)]
struct Parser {
    report: BlameReport,
    commits: HashMap<String, u32>,
    sources: HashMap<(u32, String), u32>,
    /// The last file each commit was reported with; porcelain names it only once.
    files: HashMap<u32, (String, Option<PreviousFile>)>,
    commit: u32,
    orig_line: u32,
    final_line: u32,
    previous: Option<PreviousFile>,
}

fn parse_porcelain(stdout: &str) -> BlameReport {
    let mut parser = Parser::default();
    for line in stdout.split('\n') {
        if let Some(text) = line.strip_prefix('\t') {
            parser.push_line(text);
        } else if !parser.header(line) {
            parser.detail(line);
        }
    }
    parser.report
}

impl Parser {
    fn header(&mut self, line: &str) -> bool {
        let mut fields = line.split(' ');
        let (Some(oid), Some(orig), Some(fin)) = (fields.next(), fields.next(), fields.next())
        else {
            return false;
        };
        if oid.len() != 40 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        let (Ok(orig), Ok(fin)) = (orig.parse(), fin.parse()) else {
            return false;
        };
        self.commit = self.commit_index(oid);
        self.orig_line = orig;
        self.final_line = fin;
        self.previous = None;
        true
    }

    fn commit_index(&mut self, oid: &str) -> u32 {
        if let Some(index) = self.commits.get(oid) {
            return *index;
        }
        let index = u32::try_from(self.report.commits.len()).unwrap_or(u32::MAX);
        self.report.commits.push(BlameCommit {
            oid: oid.to_owned(),
            summary: String::new(),
            author: String::new(),
            email: String::new(),
            timestamp: 0,
            merge: false,
            boundary: false,
            uncommitted: oid == UNCOMMITTED,
        });
        self.commits.insert(oid.to_owned(), index);
        index
    }

    fn detail(&mut self, line: &str) {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "previous" {
            if let Some((oid, path)) = value.split_once(' ') {
                self.previous = Some(PreviousFile {
                    oid: oid.to_owned(),
                    path: unquote_path(path),
                });
            }
            return;
        }
        if key == "filename" {
            let file = (unquote_path(value), self.previous.take());
            self.files.insert(self.commit, file);
            return;
        }
        let Some(commit) = self.report.commits.get_mut(self.commit as usize) else {
            return;
        };
        match key {
            "author" => commit.author = value.to_owned(),
            "author-mail" => {
                commit.email = value
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_owned();
            }
            "author-time" => commit.timestamp = value.parse().unwrap_or_default(),
            "summary" => commit.summary = value.to_owned(),
            "boundary" => commit.boundary = true,
            _ => {}
        }
    }

    fn push_line(&mut self, text: &str) {
        let (path, previous) = self.files.get(&self.commit).cloned().unwrap_or_default();
        let key = (self.commit, path);
        let source = match self.sources.get(&key) {
            Some(index) => *index,
            None => {
                let index = u32::try_from(self.report.sources.len()).unwrap_or(u32::MAX);
                self.report.sources.push(BlameSource {
                    commit: self.commit,
                    path: key.1.clone(),
                    previous,
                });
                self.sources.insert(key, index);
                index
            }
        };
        self.report.lines.push(OriginLine {
            line: self.final_line,
            text: text.strip_suffix('\r').unwrap_or(text).to_owned(),
            source,
            orig_line: self.orig_line,
            change: LineChange::Added,
        });
        self.final_line += 1;
        self.orig_line += 1;
    }
}

/// Undoes git's C-style quoting of a path that holds `"`, `\` or control characters.
fn unquote_path(raw: &str) -> String {
    let Some(inner) = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return raw.to_owned();
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.bytes().peekable();
    while let Some(byte) = chars.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        match chars.next() {
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(b'r') => bytes.push(b'\r'),
            Some(digit @ b'0'..=b'7') => {
                let mut value = u32::from(digit - b'0');
                for _ in 0..2 {
                    if let Some(next @ b'0'..=b'7') = chars.peek().copied() {
                        value = value * 8 + u32::from(next - b'0');
                        chars.next();
                    }
                }
                bytes.push(u8::try_from(value).unwrap_or(b'?'));
            }
            Some(other) => bytes.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_path_is_left_alone() {
        assert_eq!(unquote_path("dir/naïve name.txt"), "dir/naïve name.txt");
    }

    #[test]
    fn a_quoted_path_loses_its_quotes_and_escapes() {
        assert_eq!(unquote_path(r#""a\"b\\c\td""#), "a\"b\\c\td");
        assert_eq!(unquote_path(r#""na\303\257ve""#), "naïve");
    }

    #[test]
    fn a_group_without_a_filename_reuses_the_commit_one() {
        let oid = "a".repeat(40);
        let text = format!(
            "{oid} 1 1 2\nauthor A\nsummary s\nprevious {} old.txt\nfilename new.txt\n\tone\n\
             {oid} 2 2\n\ttwo\n",
            "b".repeat(40)
        );

        let report = parse_porcelain(&text);

        assert_eq!(report.lines.len(), 2);
        assert_eq!(report.lines[1].source, 0);
        assert_eq!(report.sources[0].path, "new.txt");
        assert_eq!(
            report.sources[0].previous.as_ref().map(|p| p.path.as_str()),
            Some("old.txt")
        );
    }
}
