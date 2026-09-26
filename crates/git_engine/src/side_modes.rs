use crate::{DiffSpec, RepoHandle};

impl RepoHandle {
    /// The mode of the path on each side of a diff as git prints it (`100644`), `None` where
    /// the side has no such entry. `old_path` is where the old side has it after a rename.
    #[must_use]
    pub fn side_modes(
        &self,
        spec: &DiffSpec,
        old_path: &str,
        path: &str,
    ) -> (Option<String>, Option<String>) {
        match spec {
            DiffSpec::CommitVsParent { oid } => {
                let parent = self.first_parent(oid).ok().flatten();
                (
                    parent.and_then(|parent| self.mode_at(&parent, old_path)),
                    self.mode_at(oid, path),
                )
            }
            DiffSpec::CommitVsCommit { a, b } => (self.mode_at(a, old_path), self.mode_at(b, path)),
            DiffSpec::IndexVsHead => (self.mode_at("HEAD", old_path), self.mode_in_index(path)),
            DiffSpec::WorkTreeVsIndex => (self.mode_in_index(old_path), self.mode_on_disk(path)),
            DiffSpec::CommitVsWorkTree { oid } => {
                (self.mode_at(oid, old_path), self.mode_on_disk(path))
            }
        }
    }

    fn mode_at(&self, rev: &str, path: &str) -> Option<String> {
        let id = self.repo.rev_parse_single(rev).ok()?;
        let tree = self.repo.find_commit(id.detach()).ok()?.tree().ok()?;
        let entry = tree.lookup_entry_by_path(path).ok()??;
        Some(entry.mode().kind().as_octal_str().to_string())
    }

    fn mode_in_index(&self, path: &str) -> Option<String> {
        let index = self.repo.index_or_empty().ok()?;
        let mode = index
            .entry_by_path(path.into())?
            .mode
            .to_tree_entry_mode()?;
        Some(mode.kind().as_octal_str().to_string())
    }

    /// As `git diff` sees the file: its executable bit counts only where the file system
    /// keeps one and `core.fileMode` is on; otherwise the index's mode stands.
    fn mode_on_disk(&self, path: &str) -> Option<String> {
        let file = self.root().join(path);
        let meta = std::fs::symlink_metadata(&file).ok()?;
        if meta.file_type().is_symlink() {
            return Some("120000".to_owned());
        }
        if !meta.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let honoured = self
                .repo
                .config_snapshot()
                .boolean("core.fileMode")
                .unwrap_or(true);
            if honoured {
                let executable = meta.permissions().mode() & 0o111 != 0;
                return Some(if executable { "100755" } else { "100644" }.to_owned());
            }
        }
        Some(
            self.mode_in_index(path)
                .unwrap_or_else(|| "100644".to_owned()),
        )
    }
}
