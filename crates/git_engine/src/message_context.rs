use crate::{RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const TOTAL_LINES: u32 = 8_000;

const LOCKFILES: &[&str] = &[
    "cargo.lock",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "bun.lock",
    "composer.lock",
    "gemfile.lock",
    "poetry.lock",
    "pipfile.lock",
    "uv.lock",
    "pdm.lock",
    "go.sum",
    "flake.lock",
    "packages.lock.json",
    "pubspec.lock",
    "mix.lock",
    "podfile.lock",
    "package.resolved",
    "gradle.lockfile",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Omitted {
    Binary,
    Lockfile,
    OverBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub added: Option<u32>,
    pub deleted: Option<u32>,
    pub omitted: Option<Omitted>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageContext {
    pub branch: Option<String>,
    pub subjects: Vec<String>,
    pub files: Vec<StagedFile>,
    pub diff: String,
}

#[must_use]
pub fn omitted_by_name(path: &str) -> Option<Omitted> {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    if LOCKFILES.contains(&name.as_str()) || name.ends_with(".lock") {
        return Some(Omitted::Lockfile);
    }
    None
}

#[must_use]
pub fn parse_staged_numstat(text: &str) -> Vec<StagedFile> {
    let mut files = Vec::new();
    let mut fields = text.split('\0');
    while let Some(head) = fields.next() {
        let head = head.trim_start_matches('\n');
        if head.is_empty() {
            continue;
        }
        let mut parts = head.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (old_path, path) = if path.is_empty() {
            let old = fields.next().unwrap_or_default().to_owned();
            let new = fields.next().unwrap_or_default().to_owned();
            (Some(old), new)
        } else {
            (None, path.to_owned())
        };
        files.push(StagedFile {
            path,
            old_path,
            added: added.parse().ok(),
            deleted: deleted.parse().ok(),
            omitted: None,
        });
    }
    files
}

fn plan_files(mut files: Vec<StagedFile>, binary: &HashSet<&str>) -> Vec<StagedFile> {
    for file in &mut files {
        let paths = std::iter::once(file.path.as_str()).chain(file.old_path.as_deref());
        file.omitted = paths.clone().find_map(omitted_by_name);
        if file.omitted.is_none()
            && (file.added.is_none() || paths.into_iter().any(|path| binary.contains(path)))
        {
            file.omitted = Some(Omitted::Binary);
        }
    }
    let mut text: Vec<_> = files
        .iter_mut()
        .filter(|file| file.omitted.is_none())
        .collect();
    let lines = |file: &StagedFile| {
        file.added
            .unwrap_or(0)
            .saturating_add(file.deleted.unwrap_or(0))
    };
    text.sort_by_key(|file| std::cmp::Reverse(lines(file)));
    let mut spent = 0_u32;
    for file in text {
        if spent >= TOTAL_LINES && lines(file) > 0 {
            file.omitted = Some(Omitted::OverBudget);
        } else {
            spent = spent.saturating_add(lines(file));
        }
    }
    files
}

impl RepoHandle {
    pub fn message_context(&self, subjects: usize) -> Result<MessageContext> {
        let numstat = self.read_git(&[
            "-c",
            "core.quotePath=false",
            "diff",
            "--cached",
            "--numstat",
            "-z",
            "-M",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ])?;
        let files = parse_staged_numstat(&numstat);
        let paths = files
            .iter()
            .flat_map(|file| std::iter::once(file.path.as_str()).chain(file.old_path.as_deref()))
            .flat_map(|path| path.bytes().chain(std::iter::once(0)))
            .collect::<Vec<_>>();
        let attributes = if paths.is_empty() {
            String::new()
        } else {
            self.read_git_fed(&["check-attr", "--cached", "-z", "--stdin", "diff"], &paths)?
        };
        let mut attrs = attributes.split_terminator('\0');
        let mut binary = HashSet::new();
        while let (Some(path), Some(_), Some(value)) = (attrs.next(), attrs.next(), attrs.next()) {
            if value == "unset" {
                binary.insert(path);
            }
        }
        let files = plan_files(files, &binary);
        let mut shown: Vec<String> = Vec::new();
        for file in files.iter().filter(|file| file.omitted.is_none()) {
            if let Some(old) = &file.old_path {
                shown.push(old.clone());
            }
            shown.push(file.path.clone());
        }
        let mut diff = String::new();
        for batch in crate::runner::command_line_batches(&shown) {
            let mut args = vec![
                "-c",
                "core.quotePath=false",
                "diff",
                "--cached",
                "-M",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=none",
                "--unified=3",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                "--submodule=short",
                "--",
            ];
            args.extend(batch.iter().map(String::as_str));
            diff.push_str(&self.read_git_literal(&args)?);
        }
        let branch = self
            .repo
            .head_name()
            .map_err(|err| crate::GitError::Internal(format!("Cannot read HEAD: {err}")))?
            .map(|name| name.shorten().to_string());
        let subjects = self
            .recent_commits(subjects)?
            .into_iter()
            .map(|commit| commit.summary)
            .filter(|subject| !subject.trim().is_empty())
            .collect();
        Ok(MessageContext {
            branch,
            subjects,
            files,
            diff,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, added: Option<u32>, deleted: Option<u32>) -> StagedFile {
        StagedFile {
            path: path.to_owned(),
            old_path: None,
            added,
            deleted,
            omitted: None,
        }
    }

    #[test]
    fn numstat_reads_renames_and_binary_files() {
        let text = "3\t1\tsrc/a.rs\0-\t-\tlogo.png\x000\t0\t\0old.txt\0new.txt\0";
        let files = parse_staged_numstat(text);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0], file("src/a.rs", Some(3), Some(1)));
        assert_eq!(files[1], file("logo.png", None, None));
        assert_eq!(files[2].path, "new.txt");
        assert_eq!(files[2].old_path.as_deref(), Some("old.txt"));
    }

    #[test]
    fn lockfiles_go_by_name() {
        assert_eq!(omitted_by_name("Cargo.lock"), Some(Omitted::Lockfile));
        assert_eq!(
            omitted_by_name("web/package-lock.json"),
            Some(Omitted::Lockfile)
        );
        assert_eq!(omitted_by_name("src/lock.rs"), None);
    }

    #[test]
    fn the_plan_keeps_files_until_the_total_is_spent() {
        let mut files = vec![
            file("a.rs", Some(10), Some(0)),
            file("b.png", None, None),
            file("Cargo.lock", Some(300), Some(200)),
            file("huge.rs", Some(4_000), Some(0)),
        ];
        for index in 0..6 {
            files.push(file(&format!("part{index}.rs"), Some(1_500), Some(0)));
        }
        files.push(file("small.rs", Some(1), Some(0)));
        let reasons: Vec<Option<Omitted>> = plan_files(files, &HashSet::new())
            .iter()
            .map(|file| file.omitted)
            .collect();
        assert_eq!(
            reasons,
            [
                Some(Omitted::OverBudget),
                Some(Omitted::Binary),
                Some(Omitted::Lockfile),
                None,
                None,
                None,
                None,
                Some(Omitted::OverBudget),
                Some(Omitted::OverBudget),
                Some(Omitted::OverBudget),
                Some(Omitted::OverBudget),
            ]
        );
    }
}
