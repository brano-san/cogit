use crate::{RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileChange {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    /// A type change, or a status this reader does not know yet.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileRevision {
    pub oid: String,
    pub parents: Vec<String>,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    /// The name the file had at this commit.
    pub path: String,
    /// The name before this commit, for a rename or a copy.
    pub previous_path: Option<String>,
    pub change: FileChange,
}

const RECORD: char = '\u{1e}';
const FIELD: char = '\u{1f}';

impl RepoHandle {
    /// Newest first, starting at `rev` (HEAD when `None`); `follow` crosses renames the
    /// way `git log --follow` does, which is why this read goes through the CLI (R-280).
    pub fn file_log(
        &self,
        path: &str,
        rev: Option<&str>,
        follow: bool,
        limit: usize,
    ) -> Result<Vec<FileRevision>> {
        if rev.is_none() && self.repo.head_id().is_err() {
            return Ok(Vec::new());
        }

        let count = format!("-{}", limit.clamp(1, 100_000));
        let format = format!("--format={RECORD}%H{FIELD}%P{FIELD}%an{FIELD}%ae{FIELD}%at{FIELD}%s");
        let mut args = vec![
            "-c",
            "core.quotepath=off",
            "log",
            "-M",
            "-z",
            "--name-status",
            "--no-color",
            &count,
            &format,
        ];
        if follow {
            args.push("--follow");
        }
        args.push(rev.unwrap_or("HEAD"));
        args.extend(["--", path]);

        Ok(parse_file_log(&self.read_git_literal(&args)?))
    }
}

fn parse_file_log(stdout: &str) -> Vec<FileRevision> {
    stdout
        .split(RECORD)
        .filter_map(|record| {
            let (header, changes) = record.split_once('\0').unwrap_or((record, ""));
            let mut revision = revision_of(header)?;
            apply_status(&mut revision, changes);
            Some(revision)
        })
        .collect()
}

fn revision_of(header: &str) -> Option<FileRevision> {
    let mut fields = header.splitn(6, FIELD);
    let oid = fields.next()?.trim().to_owned();
    if oid.is_empty() {
        return None;
    }
    let parents = fields
        .next()?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let author = fields.next()?.to_owned();
    let email = fields.next()?.to_owned();
    let timestamp = fields.next()?.parse().ok()?;
    let summary = fields.next().unwrap_or_default().trim_end().to_owned();
    Some(FileRevision {
        oid,
        parents,
        summary,
        author,
        email,
        timestamp,
        path: String::new(),
        previous_path: None,
        change: FileChange::Other,
    })
}

/// `-z --name-status` writes `M\0path\0`, or `R100\0old\0new\0` for a rename.
fn apply_status(revision: &mut FileRevision, changes: &str) {
    let mut tokens = changes
        .split('\0')
        .map(|token| token.trim_start_matches('\n'))
        .filter(|token| !token.is_empty());
    let Some(status) = tokens.next() else {
        return;
    };
    revision.change = match status.chars().next() {
        Some('A') => FileChange::Added,
        Some('M') => FileChange::Modified,
        Some('D') => FileChange::Deleted,
        Some('R') => FileChange::Renamed,
        Some('C') => FileChange::Copied,
        _ => FileChange::Other,
    };
    let first = tokens.next().unwrap_or_default().to_owned();
    if matches!(revision.change, FileChange::Renamed | FileChange::Copied) {
        revision.path = tokens.next().unwrap_or_default().to_owned();
        revision.previous_path = Some(first);
    } else {
        revision.path = first;
    }
}
