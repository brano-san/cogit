//! The graph filter's free text, matched in the fields its switches pick (F-560).

use std::collections::HashMap;
use std::ops::ControlFlow;

use gix::bstr::ByteSlice as _;
use serde::Deserialize;

use crate::{CommitQuery, CommitRow, Mailmap, RepoHandle};

/// Where the free text of the filter is looked for; any one field matching is enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TextFields {
    pub author: bool,
    pub committer: bool,
    /// The whole message, body included.
    pub message: bool,
    pub refs: bool,
    /// A prefix of the commit id.
    pub id: bool,
    /// Paths the commit changed against its first parent: the file name, or the whole path
    /// once the text has a `/` in it, as SmartGit matches.
    pub name: bool,
    /// Lines the commit added or removed against its first parent.
    pub content: bool,
}

impl Default for TextFields {
    fn default() -> Self {
        Self {
            author: true,
            committer: true,
            message: true,
            refs: true,
            id: true,
            name: false,
            content: false,
        }
    }
}

/// Binary content is not searched: git's own test, a NUL in the first 8000 bytes.
const BINARY_PROBE: usize = 8000;

/// A filter's text, read once per walk with the ref names it may match.
#[derive(Debug)]
pub(crate) struct TextMatch {
    needle: String,
    fields: TextFields,
    refs: HashMap<gix::ObjectId, Vec<String>>,
}

fn lower(text: &str) -> String {
    text.to_lowercase()
}

impl RepoHandle {
    pub(crate) fn text_match(&self, query: &CommitQuery) -> Option<TextMatch> {
        let needle = lower(query.text.as_deref()?.trim());
        if needle.is_empty() {
            return None;
        }
        let refs = if query.text_in.refs {
            self.ref_names_by_commit()
        } else {
            HashMap::new()
        };
        Some(TextMatch {
            needle,
            fields: query.text_in,
            refs,
        })
    }

    /// Branch, remote branch and tag names by the commit they peel to, in lower case.
    fn ref_names_by_commit(&self) -> HashMap<gix::ObjectId, Vec<String>> {
        let mut names: HashMap<gix::ObjectId, Vec<String>> = HashMap::new();
        let Ok(platform) = self.repo.references() else {
            return names;
        };
        let Ok(all) = platform.all() else {
            return names;
        };
        for mut reference in all.flatten() {
            let full = reference.name().as_bstr().to_string();
            if !["refs/heads/", "refs/remotes/", "refs/tags/"]
                .iter()
                .any(|prefix| full.starts_with(prefix))
            {
                continue;
            }
            let short = lower(&reference.name().shorten().to_string());
            if let Ok(commit) = reference.peel_to_commit() {
                names.entry(commit.id).or_default().push(short);
            }
        }
        names
    }
}

impl TextMatch {
    pub(crate) fn matches(
        &self,
        handle: &RepoHandle,
        id: gix::ObjectId,
        row: &CommitRow,
        mailmap: &Mailmap,
    ) -> bool {
        let needle = self.needle.as_str();
        let has = |text: &str| lower(text).contains(needle);
        let fields = self.fields;
        if fields.id && row.oid.starts_with(needle) {
            return true;
        }
        if fields.refs
            && self
                .refs
                .get(&id)
                .is_some_and(|names| names.iter().any(|name| name.contains(needle)))
        {
            return true;
        }
        if fields.author && (has(&row.author_name) || has(&row.author_email)) {
            return true;
        }
        if (fields.committer || fields.message)
            && let Ok(commit) = handle.repo.find_commit(id)
        {
            if fields.message && has(&commit.message_raw_sloppy().to_str_lossy()) {
                return true;
            }
            if fields.committer
                && let Ok(committer) = commit.committer()
            {
                let (mut name, mut email) =
                    (committer.name.to_string(), committer.email.to_string());
                mailmap.apply(&mut name, &mut email);
                if has(&name) || has(&email) {
                    return true;
                }
            }
        }
        (fields.name || fields.content) && self.matches_changes(handle, id)
    }

    fn matches_changes(&self, handle: &RepoHandle, id: gix::ObjectId) -> bool {
        let repo = &handle.repo;
        let Ok(commit) = repo.find_commit(id) else {
            return false;
        };
        let Ok(tree) = commit.tree() else {
            return false;
        };
        let before = match commit.parent_ids().next() {
            Some(parent) => {
                let tree = repo
                    .find_commit(parent.detach())
                    .map_err(|err| err.to_string())
                    .and_then(|parent| parent.tree().map_err(|err| err.to_string()));
                match tree {
                    Ok(tree) => tree,
                    Err(err) => {
                        tracing::error!(error = ?err, context = "text search: parent tree");
                        return false;
                    }
                }
            }
            None => repo.empty_tree(),
        };
        let path_wide = self.needle.contains('/');
        let mut found = false;
        let Ok(mut changes) = before.changes() else {
            return false;
        };
        let walked = changes
            .options(|options| {
                options.track_path();
                options.track_rewrites(None);
            })
            .for_each_to_obtain_tree(&tree, |change| {
                use gix::object::tree::diff::Change;
                let (old, new) = match &change {
                    Change::Addition { entry_mode, id, .. } => {
                        (None, entry_mode.is_blob().then(|| id.detach()))
                    }
                    Change::Deletion { entry_mode, id, .. } => {
                        (entry_mode.is_blob().then(|| id.detach()), None)
                    }
                    Change::Modification {
                        previous_entry_mode,
                        previous_id,
                        entry_mode,
                        id,
                        ..
                    } => (
                        previous_entry_mode.is_blob().then(|| previous_id.detach()),
                        entry_mode.is_blob().then(|| id.detach()),
                    ),
                    Change::Rewrite { .. } => (None, None),
                };
                if old.is_none() && new.is_none() {
                    return Ok::<_, std::convert::Infallible>(ControlFlow::Continue(()));
                }
                if self.fields.name {
                    let path = lower(&change.location().to_str_lossy());
                    let name = if path_wide {
                        path.as_str()
                    } else {
                        path.rsplit('/').next().unwrap_or(&path)
                    };
                    found = name.contains(self.needle.as_str());
                }
                if !found && self.fields.content {
                    found = self.changes_line(repo, old, new);
                }
                Ok(if found {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            });
        // A match stops the diff, which gix reports as cancelled.
        if let Err(err) = walked
            && !found
        {
            tracing::error!(error = ?err, context = "text search: tree diff");
        }
        found
    }

    /// Whether a line with the text is in one version and not the other: the lines are
    /// compared as a multiset, so a line only moved inside the file is no change.
    fn changes_line(
        &self,
        repo: &gix::Repository,
        old: Option<gix::ObjectId>,
        new: Option<gix::ObjectId>,
    ) -> bool {
        let needle = self.needle.as_bytes();
        let lines = |blob: Option<gix::ObjectId>| -> Vec<Vec<u8>> {
            let Some(data) = blob.and_then(|id| repo.find_blob(id).ok()) else {
                return Vec::new();
            };
            let data = &data.data;
            if data[..data.len().min(BINARY_PROBE)].contains(&0) {
                return Vec::new();
            }
            let folded = data.to_ascii_lowercase();
            if folded.find(needle).is_none() {
                return Vec::new();
            }
            let mut lines: Vec<Vec<u8>> = folded
                .lines()
                .filter(|line| line.find(needle).is_some())
                .map(<[u8]>::to_vec)
                .collect();
            lines.sort_unstable();
            lines
        };
        lines(old) != lines(new)
    }
}
