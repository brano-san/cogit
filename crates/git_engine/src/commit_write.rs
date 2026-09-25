use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    pub message: String,
    pub amend: bool,
    pub no_verify: bool,
    /// Empty means everything staged; a list narrows the commit to those paths (T6.8).
    #[serde(default)]
    pub only: Vec<String>,
}

impl RepoHandle {
    pub fn commit(&self, request: &CommitRequest) -> Result<String> {
        if request.message.trim().is_empty() {
            return Err(GitError::InvalidState(
                "a commit message cannot be empty".to_owned(),
            ));
        }

        // Cogit runs it after the commit returns (`maintain_after_commit`).
        let mut args = vec!["-c", "maintenance.auto=false", "commit"];
        if request.amend {
            args.push("--amend");
        }
        if request.no_verify {
            args.push("--no-verify");
        }
        // `-m` takes the next argument verbatim, so a message starting with `--` is safe.
        args.push("-m");
        args.push(&request.message);
        if request.only.is_empty() {
            self.run_git(&args)?;
        } else {
            self.commit_from_index(&args, &request.only)?;
        }
        // Through gix: `rev-parse HEAD` was a second process (R-313).
        let oid = self
            .repo
            .head_id()
            .map_err(|err| GitError::Internal(format!("cannot read the new HEAD: {err}")))?
            .to_string();
        if request.no_verify {
            self.record_bypass(&oid, &request.message);
        }
        self.maintain_after_commit();
        Ok(oid)
    }

    /// `commit --only` takes the named paths from the working tree, so a file staged in
    /// part (Stage lines, an edit after `add`) went in with its unstaged edits. The commit
    /// is built in a scratch index instead: HEAD, plus what the real index holds at those
    /// paths. The real index then already matches the new HEAD there and stays untouched.
    fn commit_from_index(&self, commit: &[&str], paths: &[String]) -> Result<()> {
        static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let scratch = Scratch(
            self.repo
                .index_path()
                .with_file_name(format!("cogit-only-index-{}-{run}", std::process::id())),
        );
        let base: &[&str] = if matches!(self.head()?, crate::Head::Unborn { .. }) {
            &["read-tree", "--empty"]
        } else {
            &["read-tree", "HEAD"]
        };
        self.run_git_indexed(&scratch.0, base, None)?;
        let entries = self.index_entries_of(&self.with_rename_sources(paths)?)?;
        self.run_git_indexed(
            &scratch.0,
            &["update-index", "-z", "--index-info"],
            Some(&entries),
        )?;
        self.run_git_indexed(&scratch.0, commit, None).map(drop)
    }

    /// `update-index --index-info` input: each path cleared, then given every entry the
    /// real index has for it — conflict stages too, so git refuses those as it would.
    fn index_entries_of(&self, paths: &[String]) -> Result<Vec<u8>> {
        let null = self.repo.object_hash().null().to_string();
        let mut entries = Vec::new();
        for path in paths {
            entries.extend_from_slice(format!("0 {null}\t{path}\0").as_bytes());
        }
        for batch in crate::runner::command_line_batches(paths) {
            let mut args = vec!["ls-files", "--stage", "-z", "--"];
            args.extend(batch.iter().map(String::as_str));
            entries.extend(self.run_git_bytes_literal(&args)?);
        }
        Ok(entries)
    }
}

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl RepoHandle {
    /// `commit.template` as text, or `None` when it is unset or points nowhere.
    pub fn commit_template(&self) -> Result<Option<String>> {
        let Some(path) = self.pathname_setting("commit.template") else {
            return Ok(None);
        };
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root().join(path)
        };
        Ok(std::fs::read_to_string(resolved).ok())
    }
}
