use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line: u32,
    pub text: String,
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

/// How many ignored commits deep to keep walking before giving up. A reformatting pass on
/// top of a reformatting pass is normal; a hundred of them is a loop.
const MAX_IGNORE_HOPS: usize = 16;

impl RepoHandle {
    /// One entry per line, in file order, with `.git-blame-ignore-revs` honoured.
    pub fn blame(&self, path: &str, rev: &str) -> Result<Vec<BlameLine>> {
        let mut lines = self.blame_raw(path, rev)?;
        let ignored = self.ignored_revs();
        if !ignored.is_empty() {
            self.skip_ignored(path, &mut lines, &ignored);
        }
        Ok(lines)
    }

    /// The file as it stood before `oid` touched it, or `None` when there was no such file
    /// — because the commit added it, or because it is the root commit and there is no
    /// "before" at all.
    pub fn file_before(&self, oid: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let commit = self
            .repo
            .rev_parse_single(oid)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {oid}: {err}")))?;

        let has_parent = commit
            .object()
            .ok()
            .and_then(|object| object.try_into_commit().ok())
            .is_some_and(|commit| commit.parent_ids().next().is_some());
        if !has_parent {
            return Ok(None);
        }

        self.blob_at(&format!("{oid}^"), path)
    }

    /// The commits `.git-blame-ignore-revs` asks to look past. Missing file, unreadable
    /// file and malformed lines all mean "nothing to skip": losing blame over a typo in an
    /// optional file would be worse than ignoring the file.
    fn ignored_revs(&self) -> std::collections::HashSet<String> {
        let Some(root) = self.repo.workdir() else {
            return std::collections::HashSet::new();
        };
        let Ok(text) = std::fs::read_to_string(root.join(".git-blame-ignore-revs")) else {
            return std::collections::HashSet::new();
        };
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| gix::ObjectId::from_hex(line.as_bytes()).ok())
            .map(|oid| oid.to_string())
            .collect()
    }

    /// Re-attributes every line an ignored commit claimed to whoever the same line belonged
    /// to in that commit's parent.
    ///
    /// Line numbers are matched by position, which holds for the cosmetic commits the file
    /// is meant to list — a reformatting pass keeps the lines and their order (R-103). The
    /// text stays the one being viewed; only the attribution moves.
    fn skip_ignored(
        &self,
        path: &str,
        lines: &mut [BlameLine],
        ignored: &std::collections::HashSet<String>,
    ) {
        let mut cache: std::collections::HashMap<String, Vec<BlameLine>> =
            std::collections::HashMap::new();

        for (index, line) in lines.iter_mut().enumerate() {
            for _ in 0..MAX_IGNORE_HOPS {
                if !ignored.contains(&line.oid) {
                    break;
                }
                let parent = format!("{}^", line.oid);
                if !cache.contains_key(&parent) {
                    // A root commit has no parent, and then there is nowhere left to look.
                    let Ok(older) = self.blame_raw(path, &parent) else {
                        break;
                    };
                    cache.insert(parent.clone(), older);
                }
                let Some(older) = cache.get(&parent).and_then(|older| older.get(index)) else {
                    break;
                };
                line.oid = older.oid.clone();
                line.summary = older.summary.clone();
                line.author = older.author.clone();
                line.email = older.email.clone();
                line.timestamp = older.timestamp;
            }
        }
    }

    fn blame_raw(&self, path: &str, rev: &str) -> Result<Vec<BlameLine>> {
        let suspect = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?
            .detach();

        let outcome = self
            .repo
            .blame_file(
                path.into(),
                suspect,
                gix::repository::blame_file::Options::default(),
            )
            .map_err(|err| GitError::InvalidState(format!("cannot blame {path}: {err}")))?;

        let text = String::from_utf8_lossy(&outcome.blob);
        let lines: Vec<&str> = text.lines().collect();
        let mut out: Vec<BlameLine> = Vec::with_capacity(lines.len());

        // Empty when there is no `.mailmap`, and empty again when the file is unparsable:
        // a mistyped mapping must not cost the user their blame.
        let mailmap = self.repo.open_mailmap();

        for entry in &outcome.entries {
            let oid = entry.commit_id.to_string();
            let details = self.commit_details(&oid)?;
            let (author, email) = canonical(&mailmap, &details.author);
            for offset in 0..entry.len.get() {
                let index = (entry.start_in_blamed_file + offset) as usize;
                out.push(BlameLine {
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    text: lines.get(index).copied().unwrap_or_default().to_owned(),
                    oid: oid.clone(),
                    summary: details.summary.clone(),
                    author: author.clone(),
                    email: email.clone(),
                    timestamp: details.author.timestamp,
                });
            }
        }

        out.sort_by_key(|line| line.line);
        Ok(out)
    }
}

/// The name and address `.mailmap` wants shown, or the ones the commit carries when the
/// file says nothing about them.
fn canonical(mailmap: &gix::mailmap::Snapshot, author: &crate::Signature) -> (String, String) {
    let resolved = mailmap.resolve(gix::actor::SignatureRef {
        name: gix::bstr::BStr::new(author.name.as_bytes()),
        email: gix::bstr::BStr::new(author.email.as_bytes()),
        time: "",
    });
    (resolved.name.to_string(), resolved.email.to_string())
}
