//! `diff.<driver>.textconv`: a program that turns a binary format into text to diff.

use crate::{DiffAttributes, GitError, RepoHandle, Result};
use std::io::Write;

impl RepoHandle {
    /// The command `.gitattributes` `diff=<driver>` names for `path`, if it has one.
    #[must_use]
    pub fn textconv_command(
        &self,
        attributes: &mut DiffAttributes<'_>,
        path: &str,
    ) -> Option<String> {
        let driver = attributes.value(path, "diff")?;
        let key = format!("diff.{driver}.textconv");
        let command = self
            .repo
            .config_snapshot()
            .string(key.as_str())?
            .to_string();
        (!command.trim().is_empty()).then_some(command)
    }

    /// Runs the command as git does, through its shell with the file appended: an alias
    /// starting with `!` is exactly that, on every platform git runs on.
    pub fn textconv(&self, command: &str, path: &str, bytes: &[u8]) -> Result<Vec<u8>> {
        let suffix = std::path::Path::new(path)
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_default();
        let mut file = tempfile::Builder::new()
            .prefix("cogit-textconv-")
            .suffix(&suffix)
            .tempfile()?;
        file.write_all(bytes)?;
        file.flush()?;
        let alias = format!("alias.cogit-textconv=!{command}");
        let target = crate::slash_path(file.path());
        let mut process = self.base_git(&["-c", &alias, "cogit-textconv", &target]);
        let output = crate::children::output(&mut process)?;
        if !output.status.success() {
            return Err(GitError::InvalidState(format!(
                "textconv `{command}` failed on {path}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(output.stdout)
    }
}
