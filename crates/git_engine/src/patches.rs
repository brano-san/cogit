//! Patches out (`format-patch`, `diff --binary`) and in (`apply`, `am`), F-948.

use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl PatchFile {
    pub fn write_to(&self, directory: &std::path::Path) -> Result<std::path::PathBuf> {
        use std::io::Write as _;
        let options = gix::validate::path::component::Options {
            protect_windows: cfg!(windows),
            protect_ntfs: cfg!(windows),
            protect_hfs: cfg!(target_os = "macos"),
        };
        gix::validate::path::component(self.name.as_bytes().into(), None, options)
            .map_err(|err| GitError::InvalidState(format!("Invalid patch filename: {err}")))?;
        let target = directory.join(&self.name);
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        file.write_all(&self.bytes)?;
        file.persist(&target).map_err(|err| err.error)?;
        Ok(target)
    }
}

/// `apply` to the working tree, `apply --3way` to the index too, `am --3way` for commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PatchMode {
    WorkTree,
    Index,
    Mailbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatchStat {
    pub path: String,
    pub old_path: Option<String>,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreview {
    pub files: Vec<PatchStat>,
    pub problem: Option<String>,
    pub conflicts: bool,
}

/// A clipboard holds anything; only text with a diff in it is taken for a patch.
#[must_use]
pub fn looks_like_patch(text: &str) -> bool {
    text.lines().any(|line| {
        line.starts_with("diff --git ") || line.starts_with("+++ ") || line == "GIT binary patch"
    }) && text
        .lines()
        .any(|line| line.starts_with("@@ ") || line == "GIT binary patch")
}

/// CRLF on every line, git's header lines too, is a clipboard's conversion: git writes none.
#[must_use]
pub fn pasted_patch(text: &str) -> String {
    let lines = text.matches('\n').count();
    if lines > 0 && text.matches("\r\n").count() == lines {
        text.replace("\r\n", "\n")
    } else {
        text.to_owned()
    }
}

impl RepoHandle {
    /// Oldest first; a merge is refused, as `format-patch` would silently leave it out.
    pub fn format_patches(&self, commits: &[String]) -> Result<Vec<PatchFile>> {
        if commits.is_empty() {
            return Err(GitError::InvalidState("no commit to save".to_owned()));
        }
        let oids = commits
            .iter()
            .map(|rev| self.resolve_commit(rev).map(|id| id.to_string()))
            .collect::<Result<Vec<_>>>()?;
        let mut merges = vec!["rev-list", "--no-walk", "--merges"];
        merges.extend(oids.iter().map(String::as_str));
        if let Some(merge) = self.read_git(&merges)?.lines().next() {
            return Err(GitError::InvalidState(format!(
                "{} is a merge commit: a patch cannot carry a merge",
                &merge[..merge.len().min(8)]
            )));
        }

        let dir = tempfile::tempdir()?;
        let out = dir.path().to_string_lossy().into_owned();
        // `--root`: a root commit given alone is otherwise skipped without a word.
        let mut args = vec!["format-patch", "--root", "--no-walk", "-o", &out];
        args.extend(oids.iter().map(String::as_str));
        self.read_git(&args)?;

        let mut names = std::fs::read_dir(dir.path())?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let bytes = std::fs::read(dir.path().join(&name))?;
                Ok(PatchFile { name, bytes })
            })
            .collect()
    }

    /// `diff --binary [--cached]` of the paths; untracked ones as new files.
    pub fn worktree_patch(&self, paths: &[String], staged: bool) -> Result<Vec<u8>> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no file to save".to_owned()));
        }
        let mut patch = Vec::new();
        for batch in crate::runner::command_line_batches(paths) {
            let mut args = vec!["diff", "--binary", "--no-color", "--no-ext-diff"];
            if staged {
                args.push("--cached");
            }
            args.push("--");
            args.extend(batch.iter().map(String::as_str));
            patch.extend(self.run_git_bytes_literal(&args)?);
        }
        if !staged {
            for path in self.untracked_among(paths)? {
                patch.extend(self.new_file_patch(&path)?);
            }
        }
        if patch.is_empty() {
            return Err(GitError::InvalidState(
                "the chosen files have no changes here".to_owned(),
            ));
        }
        Ok(patch)
    }

    fn untracked_among(&self, paths: &[String]) -> Result<Vec<String>> {
        let mut tracked = std::collections::HashSet::new();
        for batch in crate::runner::command_line_batches(paths) {
            let mut args = vec!["ls-files", "-z", "--"];
            args.extend(batch.iter().map(String::as_str));
            let listed = self.read_git_literal(&args)?;
            tracked.extend(listed.split('\0').map(str::to_owned));
        }
        Ok(paths
            .iter()
            .filter(|path| !tracked.contains(*path) && self.root().join(path).is_file())
            .cloned()
            .collect())
    }

    /// `diff --no-index` exits 1 when the sides differ, which they always do here.
    fn new_file_patch(&self, path: &str) -> Result<Vec<u8>> {
        let args = [
            "diff",
            "--no-index",
            "--binary",
            "--no-color",
            "--no-ext-diff",
            "--",
            "/dev/null",
            path,
        ];
        let started = std::time::Instant::now();
        let mut command = self.base_git(&args);
        let output = crate::children::output(&mut command).map_err(crate::runner::not_started)?;
        if matches!(output.status.code(), Some(0 | 1)) {
            return Ok(output.stdout);
        }
        Err(self.failed(crate::runner::redact_command(&args), &output, started))
    }

    pub fn patch_preview(&self, text: &str, mode: PatchMode) -> Result<PatchPreview> {
        let (code, stdout, stderr) = self.quiet_apply(&["--numstat", "-z"], text)?;
        if code != Some(0) {
            return Ok(PatchPreview {
                problem: Some(stderr),
                ..PatchPreview::default()
            });
        }
        let files = parse_numstat(&stdout);
        let check: &[&str] = match mode {
            PatchMode::WorkTree => &["--check"],
            PatchMode::Index => &["--check", "--3way"],
            // `am` commits on top of HEAD and refuses a dirty index, so the index is HEAD.
            PatchMode::Mailbox => &["--check", "--3way", "--cached"],
        };
        let (code, _, stderr) = self.quiet_apply(check, text)?;
        Ok(PatchPreview {
            files,
            conflicts: code == Some(0) && stderr.contains("with conflicts"),
            problem: (code != Some(0)).then_some(stderr),
        })
    }

    /// Out of the journal: a refused check is shown in the dialog, not as an error notice.
    fn quiet_apply(&self, args: &[&str], text: &str) -> Result<(Option<i32>, String, String)> {
        let mut all = vec!["apply"];
        all.extend_from_slice(args);
        all.push("-");
        let mut command = self.base_git(&all);
        let output = crate::children::output_fed(&mut command, text.as_bytes())
            .map_err(crate::runner::not_started)?;
        let stderr = String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_owned();
        tracing::debug!(args = ?all, code = ?output.status.code(), %stderr, "patch check");
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr,
        ))
    }

    /// `--3way` and `am` may stop on conflicts and leave the rest to the user.
    pub fn apply_user_patch(&self, text: &str, mode: PatchMode) -> Result<()> {
        if text.trim().is_empty() {
            return Err(GitError::InvalidState("an empty patch".to_owned()));
        }
        let args: &[&str] = match mode {
            PatchMode::WorkTree => &["apply", "-"],
            PatchMode::Index => &["apply", "--3way", "-"],
            PatchMode::Mailbox => &["am", "--3way"],
        };
        tracing::info!(?mode, bytes = text.len(), "applying a patch");
        self.run_git_fed(args, text.as_bytes()).map(drop)
    }

    pub fn unapply_user_patch(&self, text: &str, index: bool) -> Result<()> {
        let mut args = vec!["apply", "--reverse"];
        if index {
            args.push("--index");
        }
        args.push("-");
        self.run_git_fed(&args, text.as_bytes()).map(drop)
    }
}

/// `apply --numstat -z`: `added\tremoved\tpath\0`, or for a rename an empty path followed
/// by `old\0new\0`; `-` counts for a binary file.
#[must_use]
pub fn parse_numstat(out: &str) -> Vec<PatchStat> {
    let mut fields = out.split('\0');
    let mut stats = Vec::new();
    while let Some(head) = fields.next() {
        let mut parts = head.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
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
        stats.push(PatchStat {
            path,
            old_path,
            added: added.parse().ok(),
            removed: removed.parse().ok(),
        });
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_a_patch_keeps_the_server_filename_inside_the_selected_folder() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("patches");
        std::fs::create_dir(&folder).unwrap();
        let patch = |name: &str| PatchFile {
            name: name.to_owned(),
            bytes: b"patch".to_vec(),
        };
        for name in ["../elsewhere", "/other/file", "./", ".git"] {
            assert!(patch(name).write_to(&folder).is_err(), "{name}");
        }
        assert!(!directory.path().join("elsewhere").exists());
        let saved = patch("0001-change.patch").write_to(&folder).unwrap();
        assert_eq!(std::fs::read(saved).unwrap(), b"patch");
    }

    #[test]
    fn numstat_reads_counts_binaries_and_renames() {
        let stats = parse_numstat("1\t2\ta.txt\0-\t-\tb.bin\x000\t0\t\0old.txt\0new.txt\0");
        assert_eq!(
            stats,
            vec![
                PatchStat {
                    path: "a.txt".into(),
                    old_path: None,
                    added: Some(1),
                    removed: Some(2)
                },
                PatchStat {
                    path: "b.bin".into(),
                    old_path: None,
                    added: None,
                    removed: None
                },
                PatchStat {
                    path: "new.txt".into(),
                    old_path: Some("old.txt".into()),
                    added: Some(0),
                    removed: Some(0)
                },
            ]
        );
    }

    #[test]
    fn a_patch_is_told_from_other_text() {
        assert!(looks_like_patch(
            "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n"
        ));
        assert!(!looks_like_patch("https://example.com/repo.git"));
        assert!(!looks_like_patch("+++ not a hunk"));
    }

    #[test]
    fn crlf_on_every_line_is_taken_off_and_mixed_endings_stay() {
        assert_eq!(pasted_patch("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(pasted_patch("a\nb\r\n"), "a\nb\r\n");
    }
}
