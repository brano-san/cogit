//! Canonical author identities, resolved the way `git check-mailmap` resolves them.
//! `gix::mailmap::Snapshot` drops an address an earlier line gave and rewrites the case of
//! a matched address; git does neither, so the lookup is git's own (R-390).

use crate::RepoHandle;
use gix::bstr::ByteSlice;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::SystemTime;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Rule {
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug)]
struct ByEmail {
    email: String,
    rule: Rule,
    by_name: Vec<(String, Rule)>,
}

#[derive(Debug, Default)]
pub struct Mailmap {
    entries: Vec<ByEmail>,
}

/// Entries are sorted by this: ASCII-only folding, as git's `strcasecmp`.
fn fold(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

impl Mailmap {
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Self {
        let mut map = Self::default();
        map.merge(bytes);
        map
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn merge(&mut self, bytes: &[u8]) {
        for entry in gix::mailmap::parse_ignore_errors(bytes) {
            let rule = Rule {
                name: entry
                    .new_name()
                    .map(|name| name.to_str_lossy().into_owned()),
                email: entry
                    .new_email()
                    .map(|email| email.to_str_lossy().into_owned()),
            };
            let old_email = entry.old_email().to_str_lossy();
            let old_name = entry.old_name().map(|name| name.to_str_lossy());
            self.add(&old_email, old_name.as_deref(), rule);
        }
    }

    fn add(&mut self, old_email: &str, old_name: Option<&str>, rule: Rule) {
        let at = match self
            .entries
            .binary_search_by(|entry| fold(&entry.email, old_email))
        {
            Ok(at) => at,
            Err(at) => {
                self.entries.insert(
                    at,
                    ByEmail {
                        email: old_email.to_owned(),
                        rule: Rule::default(),
                        by_name: Vec::new(),
                    },
                );
                at
            }
        };
        let entry = &mut self.entries[at];
        let Some(old_name) = old_name else {
            // Git fills in the parts a line names and keeps what an earlier line gave.
            if rule.name.is_some() {
                entry.rule.name = rule.name;
            }
            if rule.email.is_some() {
                entry.rule.email = rule.email;
            }
            return;
        };
        match entry
            .by_name
            .binary_search_by(|(name, _)| fold(name, old_name))
        {
            Ok(at) => entry.by_name[at].1 = rule,
            Err(at) => entry.by_name.insert(at, (old_name.to_owned(), rule)),
        }
    }

    /// The name and address to show for `name <email>`: git's `map_user`.
    #[must_use]
    pub fn canonical<'a>(&'a self, name: &'a str, email: &'a str) -> (&'a str, &'a str) {
        let Ok(at) = self
            .entries
            .binary_search_by(|entry| fold(&entry.email, email))
        else {
            return (name, email);
        };
        let entry = &self.entries[at];
        let rule = entry
            .by_name
            .binary_search_by(|(old, _)| fold(old, name))
            .map_or(&entry.rule, |at| &entry.by_name[at].1);
        (
            rule.name.as_deref().unwrap_or(name),
            rule.email.as_deref().unwrap_or(email),
        )
    }

    /// Rewrites a name and an address in place; nothing is allocated when neither maps.
    pub fn apply(&self, name: &mut String, email: &mut String) {
        if self.is_empty() {
            return;
        }
        let (new_name, new_email) = self.canonical(name, email);
        let (new_name, new_email) = (
            (new_name != name.as_str()).then(|| new_name.to_owned()),
            (new_email != email.as_str()).then(|| new_email.to_owned()),
        );
        if let Some(new_name) = new_name {
            *name = new_name;
        }
        if let Some(new_email) = new_email {
            *email = new_email;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn of(metadata: &std::fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

/// Equal stamps, same mailmap: a handle kept open still sees an edited `.mailmap`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    tree_file: Option<FileStamp>,
    blob: Option<gix::ObjectId>,
    file: Option<(PathBuf, Option<FileStamp>)>,
}

/// Keyed by the handle's root: a linked worktree has a `.mailmap` of its own.
type Loaded = HashMap<PathBuf, (Stamp, Arc<Mailmap>)>;
static LOADED: LazyLock<Mutex<Loaded>> = LazyLock::new(Mutex::default);

impl RepoHandle {
    /// `.mailmap` in the working tree (`HEAD:.mailmap` in a bare repository), then
    /// `mailmap.blob`, then `mailmap.file`; read again only when one of them changed.
    #[must_use]
    pub fn mailmap(&self) -> Arc<Mailmap> {
        let stamp = self.mailmap_stamp();
        let root = self.root().to_path_buf();
        if let Some((seen, map)) = LOADED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&root)
            && *seen == stamp
        {
            return Arc::clone(map);
        }

        let map = Arc::new(self.read_mailmap(&stamp));
        LOADED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(root, (stamp, Arc::clone(&map)));
        map
    }

    fn mailmap_stamp(&self) -> Stamp {
        let tree_file = self.repo.workdir().and_then(|root| {
            // Git refuses to follow a symlinked `.mailmap` in the tree.
            std::fs::symlink_metadata(root.join(".mailmap"))
                .ok()
                .filter(|metadata| !metadata.file_type().is_symlink())
                .map(|metadata| FileStamp::of(&metadata))
        });

        let config = self.repo.config_snapshot();
        let spec = config
            .string("mailmap.blob")
            .map(|spec| spec.to_str_lossy().into_owned())
            .or_else(|| {
                self.repo
                    .workdir()
                    .is_none()
                    .then(|| "HEAD:.mailmap".to_owned())
            });
        let blob = spec.and_then(|spec| {
            self.repo
                .rev_parse_single(spec.as_str())
                .map_err(|err| tracing::debug!(error = %err, spec, "no mailmap.blob"))
                .ok()
                .map(gix::Id::detach)
        });

        let file = match config.trusted_path("mailmap.file") {
            Ok(path) => path.map(|path| {
                // Git resolves a relative path against where it runs, which for every
                // command Cogit spawns is the repository root.
                let path = if path.is_relative() {
                    self.root().join(path)
                } else {
                    path
                };
                let stamp = std::fs::metadata(&path).ok().map(|m| FileStamp::of(&m));
                (path, stamp)
            }),
            Err(err) => {
                tracing::warn!(error = %err, context = "mailmap.file");
                None
            }
        };

        Stamp {
            tree_file,
            blob,
            file,
        }
    }

    fn read_mailmap(&self, stamp: &Stamp) -> Mailmap {
        let mut map = Mailmap::default();
        if stamp.tree_file.is_some()
            && let Some(root) = self.repo.workdir()
        {
            merge_file(&mut map, &root.join(".mailmap"));
        }
        if let Some(blob) = stamp.blob {
            match self.repo.find_object(blob) {
                Ok(object) if object.kind == gix::object::Kind::Blob => map.merge(&object.data),
                Ok(object) => {
                    tracing::warn!(kind = %object.kind, "mailmap.blob is not a blob");
                }
                Err(err) => tracing::warn!(error = %err, context = "mailmap.blob"),
            }
        }
        if let Some((path, Some(_))) = &stamp.file {
            merge_file(&mut map, path);
        }
        map
    }
}

fn merge_file(map: &mut Mailmap, path: &Path) {
    match std::fs::read(path) {
        Ok(bytes) => map.merge(&bytes),
        Err(err) => tracing::warn!(error = %err, path = %path.display(), "cannot read a mailmap"),
    }
}

#[cfg(test)]
mod tests {
    use super::Mailmap;

    fn shown(map: &Mailmap, name: &str, email: &str) -> (String, String) {
        let (name, email) = map.canonical(name, email);
        (name.to_owned(), email.to_owned())
    }

    #[test]
    fn a_later_name_line_keeps_the_address_an_earlier_line_gave() {
        let map = Mailmap::parse(b"<new@x> <old@x>\nNew Name <old@x>\n");

        assert_eq!(
            shown(&map, "Old", "old@x"),
            ("New Name".to_owned(), "new@x".to_owned())
        );
    }

    #[test]
    fn a_match_by_address_keeps_the_case_it_was_committed_with() {
        let map = Mailmap::parse(b"Proper <ann@x>\n");

        assert_eq!(
            shown(&map, "ann", "Ann@X"),
            ("Proper".to_owned(), "Ann@X".to_owned())
        );
    }

    #[test]
    fn a_by_name_line_wins_over_the_address_line_only_for_that_name() {
        let map = Mailmap::parse(b"Everyone <all@x>\nRobert <rob@x> Bob <all@x>\n");

        assert_eq!(
            shown(&map, "BOB", "ALL@x"),
            ("Robert".to_owned(), "rob@x".to_owned())
        );
        assert_eq!(
            shown(&map, "Carol", "all@x"),
            ("Everyone".to_owned(), "all@x".to_owned())
        );
    }

    #[test]
    fn an_address_known_only_by_name_is_left_alone_for_other_names() {
        let map = Mailmap::parse(b"Robert <rob@x> Bob <all@x>\n");

        assert_eq!(
            shown(&map, "Carol", "all@x"),
            ("Carol".to_owned(), "all@x".to_owned())
        );
    }

    #[test]
    fn apply_leaves_an_unmapped_identity_untouched() {
        let map = Mailmap::parse(b"Proper <ann@x>\n");
        let (mut name, mut email) = ("Eve".to_owned(), "eve@x".to_owned());

        map.apply(&mut name, &mut email);

        assert_eq!((name.as_str(), email.as_str()), ("Eve", "eve@x"));
    }
}
