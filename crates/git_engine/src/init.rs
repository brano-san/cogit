//! The Welcome dialog: what a picked folder is, and `git init` for one that is not a
//! repository (writes go through the system git).

use crate::{CommandSink, GitCommandError, GitError, GitOutput, RepoHandle, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FolderKind {
    /// Inside a repository's working tree, as Open Repository finds it.
    Repository,
    Plain,
    Missing,
    File,
}

#[must_use]
pub fn folder_kind(path: &Path) -> FolderKind {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => FolderKind::File,
        Ok(_) if RepoHandle::open(path).is_ok() => FolderKind::Repository,
        Ok(_) => FolderKind::Plain,
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::error!(error = ?err, path = %path.display(), context = "reading a folder's kind");
            }
            FolderKind::Missing
        }
    }
}

pub fn init_repository(path: &Path, journal: Option<&CommandSink>) -> Result<PathBuf> {
    let target = path.to_string_lossy();
    let args = ["init", "--", target.as_ref()];
    let command = crate::redact_command(&args);
    let started = std::time::Instant::now();
    tracing::info!(%command, "running git");
    let mut process = crate::runner::git_command();
    process.current_dir(std::env::temp_dir()).args(args);
    let output = crate::children::output(&mut process).map_err(crate::runner::not_started)?;
    let record = GitOutput::record(
        path,
        command,
        output.status.code(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        crate::runner::elapsed_ms(started),
    );
    if let Some(sink) = journal {
        sink(record.clone());
    }
    if output.status.success() {
        Ok(path.to_path_buf())
    } else {
        Err(GitError::Command(Box::new(GitCommandError::from_output(
            record,
        ))))
    }
}
