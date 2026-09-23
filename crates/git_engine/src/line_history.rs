use crate::{CommitQuery, CommitRow, GitError, RepoHandle, Result};
use serde::Serialize;

/// One version of a line: the commit that left it looking like this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LineVersion {
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    /// The path the file had at this commit.
    pub path: String,
    /// Where the line stood in that version, from 1.
    pub line: u32,
    pub text: String,
}

/// Starts a record; no line of a diff body can begin with it.
const VERSION: &str = "<<cogit-line>>";

impl RepoHandle {
    /// Every commit that changed line `line` of `path` as it reads at `rev`, newest first.
    ///
    /// `git log -L` carries the line through edits, moves and renames, which `gix` cannot
    /// (R-104); the same trade as Investigate, for one line (R-202).
    pub fn line_history(
        &self,
        path: &str,
        rev: &str,
        line: u32,
        limit: usize,
    ) -> Result<Vec<LineVersion>> {
        if line == 0 {
            return Err(GitError::InvalidState("line numbers start at 1".to_owned()));
        }
        if rev.starts_with('-') {
            return Err(GitError::InvalidState(format!("not a revision: {rev}")));
        }

        let range = format!("-L{line},{line}:{path}");
        let count = format!("-{}", limit.clamp(1, 1000));
        let output = self.run_git_reading(&[
            "log",
            &range,
            &count,
            "--no-color",
            &format!("--format={VERSION}%H%x09%aN%x09%aE%x09%at%x09%s"),
            rev,
        ])?;

        Ok(parse_versions(&output.stdout))
    }

    /// The commits reachable from `rev` that changed `path`, newest first: the versions of
    /// the file there are to look at.
    pub fn file_revisions(&self, path: &str, rev: &str, limit: usize) -> Result<Vec<CommitRow>> {
        let query = CommitQuery {
            path: Some(path.to_owned()),
            visible_refs: Some(vec![rev.to_owned()]),
            ..CommitQuery::default()
        };
        let mut rows = Vec::new();
        self.search_commits(&query, 100, |chunk| {
            rows.extend(chunk);
            rows.len() < limit
        })?;
        rows.truncate(limit);
        Ok(rows)
    }
}

/// What one version is being assembled from while its diff streams past.
struct Pending {
    version: LineVersion,
    in_hunk: bool,
    next_line: u32,
    added: Option<(u32, String)>,
    kept: Option<(u32, String)>,
}

fn parse_versions(stdout: &str) -> Vec<LineVersion> {
    let mut done = Vec::new();
    let mut pending: Option<Pending> = None;

    for line in stdout.lines() {
        if let Some(header) = line.strip_prefix(VERSION) {
            done.extend(pending.take().map(finish));
            pending = header_of(header).map(|version| Pending {
                version,
                in_hunk: false,
                next_line: 0,
                added: None,
                kept: None,
            });
            continue;
        }
        let Some(current) = pending.as_mut() else {
            continue;
        };
        if let Some(start) = hunk_start(line) {
            current.in_hunk = true;
            current.next_line = start;
            continue;
        }
        if !current.in_hunk {
            if current.version.path.is_empty()
                && let Some(path) = path_of(line)
            {
                current.version.path = path;
            }
            continue;
        }
        // The line as this commit left it is the one it added; a range the history has
        // widened to several lines offers its first added one.
        if let Some(text) = line.strip_prefix('+') {
            current
                .added
                .get_or_insert_with(|| (current.next_line, text.to_owned()));
            current.next_line += 1;
        } else if let Some(text) = line.strip_prefix(' ') {
            current
                .kept
                .get_or_insert_with(|| (current.next_line, text.to_owned()));
            current.next_line += 1;
        }
    }
    done.extend(pending.map(finish));
    done
}

fn finish(pending: Pending) -> LineVersion {
    let mut version = pending.version;
    if let Some((line, text)) = pending.added.or(pending.kept) {
        version.line = line;
        version.text = text;
    }
    version
}

fn header_of(header: &str) -> Option<LineVersion> {
    let mut fields = header.splitn(5, '\t');
    Some(LineVersion {
        oid: fields.next()?.to_owned(),
        author: fields.next()?.to_owned(),
        email: fields.next()?.to_owned(),
        timestamp: fields.next()?.parse().ok()?,
        summary: fields.next().unwrap_or_default().to_owned(),
        path: String::new(),
        line: 0,
        text: String::new(),
    })
}

/// The first line on the new side of `@@ -a,b +c,d @@`.
fn hunk_start(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("@@ -")?;
    let (_, new) = rest.split_once(" +")?;
    let digits: String = new.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// `+++ b/…` names the file at this commit; `--- a/…` when the commit deleted it.
fn path_of(line: &str) -> Option<String> {
    for (prefix, marker) in [("+++ ", "b/"), ("--- ", "a/")] {
        if let Some(rest) = line.strip_prefix(prefix) {
            if rest == "/dev/null" {
                continue;
            }
            return Some(rest.strip_prefix(marker).unwrap_or(rest).to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_side_of_a_hunk_header_is_read() {
        assert_eq!(hunk_start("@@ -2 +3 @@ fn main() {"), Some(3));
        assert_eq!(hunk_start("@@ -0,0 +1,220 @@"), Some(1));
        assert_eq!(hunk_start("+@@ looks like one"), None);
    }

    #[test]
    fn a_version_without_a_hunk_keeps_its_commit() {
        let parsed = parse_versions(&format!("{VERSION}abc\tA\ta@x\t10\tsubject\n"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].summary, "subject");
        assert_eq!(parsed[0].line, 0);
    }
}
