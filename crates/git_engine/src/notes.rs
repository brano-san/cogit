//! `git notes`: text attached to a commit from outside it, one namespace per `refs/notes/*`.

use crate::{RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitNote {
    /// `commits` for `refs/notes/commits`.
    pub namespace: String,
    pub text: String,
}

impl RepoHandle {
    /// Writes `refs/notes/commits`; blank text removes the note.
    pub fn set_note(&self, rev: &str, text: &str) -> Result<()> {
        let oid = self.resolve_commit(rev)?.to_string();
        if text.trim().is_empty() {
            self.run_git(&["notes", "remove", "--ignore-missing", &oid])?;
        } else {
            self.run_git_fed(&["notes", "add", "-f", "-F", "-", &oid], text.as_bytes())?;
        }
        Ok(())
    }

    pub(crate) fn notes_of(&self, oid: gix::ObjectId) -> Vec<CommitNote> {
        let Ok(platform) = self.repo.references() else {
            return Vec::new();
        };
        let Ok(refs) = platform.prefixed("refs/notes/") else {
            return Vec::new();
        };
        let hex = oid.to_string();
        let mut notes = Vec::new();
        for mut reference in refs.flatten() {
            let name = reference.name().as_bstr().to_string();
            let Some(tree) = reference.peel_to_commit().ok().and_then(|c| c.tree().ok()) else {
                continue;
            };
            if let Some(text) = self.note_in(tree, &hex) {
                notes.push(CommitNote {
                    namespace: name.trim_start_matches("refs/notes/").to_owned(),
                    text,
                });
            }
        }
        notes
    }

    /// Notes trees fan out as `ab/cdef…` once they grow; each level eats a prefix.
    fn note_in(&self, tree: gix::Tree<'_>, hex: &str) -> Option<String> {
        for entry in tree.iter().flatten() {
            let name = entry.filename().to_string();
            if entry.mode().is_blob() && name == hex {
                let blob = self.repo.find_object(entry.oid()).ok()?;
                return Some(String::from_utf8_lossy(&blob.data).trim_end().to_owned());
            }
            if entry.mode().is_tree() && hex.len() > name.len() && hex.starts_with(&name) {
                let sub = self
                    .repo
                    .find_object(entry.oid())
                    .ok()?
                    .try_into_tree()
                    .ok()?;
                return self.note_in(sub, &hex[name.len()..]);
            }
        }
        None
    }
}
