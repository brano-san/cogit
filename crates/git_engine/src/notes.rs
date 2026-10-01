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

    /// Every commit with a note in any namespace, without reading the notes themselves.
    pub fn noted_commits(&self) -> Vec<String> {
        let Ok(platform) = self.repo.references() else {
            return Vec::new();
        };
        let Ok(refs) = platform.prefixed("refs/notes/") else {
            return Vec::new();
        };
        let full = self.repo.object_hash().len_in_hex();
        let mut found = std::collections::BTreeSet::new();
        for mut reference in refs.flatten() {
            if let Some(tree) = reference.peel_to_commit().ok().and_then(|c| c.tree().ok()) {
                self.collect_noted(tree, "", full, &mut |name, _| {
                    found.insert(name);
                });
            }
        }
        found.into_iter().collect()
    }

    /// Note blobs by the commit they annotate, every namespace; the blobs stay unread.
    pub(crate) fn note_blobs(
        &self,
    ) -> std::collections::HashMap<gix::ObjectId, Vec<gix::ObjectId>> {
        let mut blobs: std::collections::HashMap<_, Vec<_>> = std::collections::HashMap::new();
        let Ok(platform) = self.repo.references() else {
            return blobs;
        };
        let Ok(refs) = platform.prefixed("refs/notes/") else {
            return blobs;
        };
        let full = self.repo.object_hash().len_in_hex();
        for mut reference in refs.flatten() {
            if let Some(tree) = reference.peel_to_commit().ok().and_then(|c| c.tree().ok()) {
                self.collect_noted(tree, "", full, &mut |name, blob| {
                    if let Ok(id) = gix::ObjectId::from_hex(name.as_bytes()) {
                        blobs.entry(id).or_default().push(blob);
                    }
                });
            }
        }
        blobs
    }

    pub(crate) fn collect_noted(
        &self,
        tree: gix::Tree<'_>,
        prefix: &str,
        full: usize,
        found: &mut impl FnMut(String, gix::ObjectId),
    ) {
        for entry in tree.iter().flatten() {
            let name = format!("{prefix}{}", entry.filename());
            if entry.mode().is_blob() && name.len() == full {
                found(name, entry.oid().to_owned());
            } else if entry.mode().is_tree()
                && name.len() < full
                && let Some(sub) = self
                    .repo
                    .find_object(entry.oid())
                    .ok()
                    .and_then(|o| o.try_into_tree().ok())
            {
                self.collect_noted(sub, &name, full, found);
            }
        }
    }

    /// The notes of one commit, every namespace.
    pub fn commit_notes(&self, rev: &str) -> Result<Vec<CommitNote>> {
        Ok(self.notes_of(self.resolve_commit(rev)?))
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
