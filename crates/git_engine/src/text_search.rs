//! The graph filter's free text, matched in the fields its switches pick (F-560).

use std::collections::HashMap;

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
    /// Git Notes of the commit, in every `refs/notes/*` namespace.
    pub notes: bool,
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
            notes: true,
        }
    }
}

/// Blobs that differ between two trees, shallowest name first; `visit` returns `true` to stop.
struct TreeWalk<'a> {
    repo: &'a gix::Repository,
    with_path: bool,
    path: Vec<u8>,
    #[allow(clippy::type_complexity)]
    visit: &'a mut dyn FnMut(&[u8], &[u8], Option<gix::ObjectId>, Option<gix::ObjectId>) -> bool,
}

fn entries<'o>(
    object: &'o Option<gix::Object<'_>>,
    kind: gix::hash::Kind,
) -> Result<Vec<gix::objs::tree::EntryRef<'o>>, String> {
    match object {
        Some(object) => gix::objs::TreeRef::from_bytes(&object.data, kind)
            .map(|tree| tree.entries)
            .map_err(|err| err.to_string()),
        None => Ok(Vec::new()),
    }
}

impl TreeWalk<'_> {
    fn diff(
        &mut self,
        before: Option<gix::ObjectId>,
        after: Option<gix::ObjectId>,
    ) -> Result<bool, String> {
        let read = |id: Option<gix::ObjectId>| {
            id.map(|id| self.repo.find_object(id).map_err(|err| err.to_string()))
                .transpose()
        };
        let (old, new) = (read(before)?, read(after)?);
        let kind = self.repo.object_hash();
        let (old, new) = (entries(&old, kind)?, entries(&new, kind)?);
        let (mut i, mut j) = (0, 0);
        while i < old.len() || j < new.len() {
            let order = match (old.get(i), new.get(j)) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Greater,
            };
            let stop = match order {
                std::cmp::Ordering::Less => {
                    i += 1;
                    self.side(&old[i - 1], true)?
                }
                std::cmp::Ordering::Greater => {
                    j += 1;
                    self.side(&new[j - 1], false)?
                }
                std::cmp::Ordering::Equal => {
                    let (a, b) = (&old[i], &new[j]);
                    i += 1;
                    j += 1;
                    if a.oid == b.oid && a.mode == b.mode {
                        false
                    } else if a.mode.is_tree() {
                        self.descend(a.filename, Some(a.oid.to_owned()), Some(b.oid.to_owned()))?
                    } else {
                        let blob = |e: &gix::objs::tree::EntryRef<'_>| {
                            e.mode.is_blob().then(|| e.oid.to_owned())
                        };
                        self.changed(a.filename, blob(a), blob(b))
                    }
                }
            };
            if stop {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// An entry only one side has: a tree is all added or all deleted.
    fn side(
        &mut self,
        entry: &gix::objs::tree::EntryRef<'_>,
        deleted: bool,
    ) -> Result<bool, String> {
        let id = entry.oid.to_owned();
        if entry.mode.is_tree() {
            let (before, after) = if deleted {
                (Some(id), None)
            } else {
                (None, Some(id))
            };
            return self.descend(entry.filename, before, after);
        }
        let blob = entry.mode.is_blob().then_some(id);
        Ok(if deleted {
            self.changed(entry.filename, blob, None)
        } else {
            self.changed(entry.filename, None, blob)
        })
    }

    fn descend(
        &mut self,
        name: &gix::bstr::BStr,
        before: Option<gix::ObjectId>,
        after: Option<gix::ObjectId>,
    ) -> Result<bool, String> {
        let keep = self.path.len();
        if self.with_path {
            self.path.extend_from_slice(name);
            self.path.push(b'/');
        }
        let stopped = self.diff(before, after);
        self.path.truncate(keep);
        stopped
    }

    fn changed(
        &mut self,
        name: &gix::bstr::BStr,
        old: Option<gix::ObjectId>,
        new: Option<gix::ObjectId>,
    ) -> bool {
        if old.is_none() && new.is_none() {
            return false;
        }
        let keep = self.path.len();
        if self.with_path {
            self.path.extend_from_slice(name);
        }
        let stop = (self.visit)(name, &self.path, old, new);
        self.path.truncate(keep);
        stop
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
    notes: HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
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
        let notes = if query.text_in.notes {
            self.note_blobs()
        } else {
            HashMap::new()
        };
        Some(TextMatch {
            needle,
            fields: query.text_in,
            refs,
            notes,
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
    /// Whether a commit costs a tree diff to test, which is worth spreading over threads.
    pub(crate) fn is_heavy(&self) -> bool {
        self.fields.name || self.fields.content
    }

    pub(crate) fn matches(
        &self,
        repo: &gix::Repository,
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
        if fields.notes
            && self.notes.get(&id).is_some_and(|blobs| {
                blobs.iter().any(|blob| {
                    repo.find_blob(*blob)
                        .is_ok_and(|blob| has(&blob.data.to_str_lossy()))
                })
            })
        {
            return true;
        }
        if fields.author && (has(&row.author_name) || has(&row.author_email)) {
            return true;
        }
        if (fields.committer || fields.message)
            && let Ok(commit) = repo.find_commit(id)
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
        (fields.name || fields.content) && self.matches_changes(repo, id)
    }

    /// A blob the commit changed against its first parent, by name or by a changed line.
    /// Walks the two trees by hand: gix's tree diff costs five times as much (R-625).
    fn matches_changes(&self, repo: &gix::Repository, id: gix::ObjectId) -> bool {
        let Ok(commit) = repo.find_commit(id) else {
            return false;
        };
        let Ok(after) = commit.tree_id() else {
            return false;
        };
        let before = match commit.parent_ids().next() {
            Some(parent) => match repo
                .find_commit(parent.detach())
                .map_err(|err| err.to_string())
                .and_then(|parent| parent.tree_id().map_err(|err| err.to_string()))
            {
                Ok(tree) => Some(tree.detach()),
                Err(err) => {
                    tracing::error!(error = ?err, context = "text search: parent tree");
                    return false;
                }
            },
            None => None,
        };
        let path_wide = self.needle.contains('/');
        let mut found = false;
        let mut visit =
            |name: &[u8], path: &[u8], old: Option<gix::ObjectId>, new: Option<gix::ObjectId>| {
                if self.fields.name {
                    let shown = if path_wide { path } else { name };
                    found = lower(&shown.to_str_lossy()).contains(self.needle.as_str());
                }
                if !found && self.fields.content {
                    found = self.changes_line(repo, old, new);
                }
                found
            };
        let mut walk = TreeWalk {
            repo,
            with_path: path_wide && self.fields.name,
            path: Vec::new(),
            visit: &mut visit,
        };
        if let Err(err) = walk.diff(before, Some(after.detach())) {
            tracing::error!(error = ?err, context = "text search: tree diff");
        }
        found
    }

    #[cfg(test)]
    fn matches_changes_gix(&self, repo: &gix::Repository, id: gix::ObjectId) -> bool {
        use std::ops::ControlFlow;
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
                    return Ok::<_, gix::Exn>(ControlFlow::Continue(()));
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use test_fixtures::Fixture;

    /// Trees against blobs of the same name, nested adds and removals, mode changes, a link
    /// and a gitlink: the shapes a hand-written tree walk can get wrong.
    fn tricky() -> Fixture {
        let f = Fixture::init().unwrap();
        let mut at = 0;
        let mut commit = |f: &Fixture, message: &str| {
            at += 1;
            f.commit_staged(at, message).unwrap();
        };
        for (path, text) in [
            ("foo", "plain foo\n"),
            ("a.b/x.txt", "dotted dir\n"),
            ("a-b", "dash file\n"),
            ("a", "just a\n"),
            ("deep/one/two/leaf.txt", "needle leaf\n"),
        ] {
            f.write_file(path, text).unwrap();
        }
        f.git(&["add", "-A"]).unwrap();
        commit(&f, "first");
        f.git(&["rm", "-q", "foo"]).unwrap();
        f.write_file("foo/inside.txt", "now a dir\n").unwrap();
        f.git(&["add", "-A"]).unwrap();
        commit(&f, "file becomes dir");
        f.git(&["rm", "-q", "-r", "foo"]).unwrap();
        f.write_file("foo", "a file again\n").unwrap();
        f.git(&["add", "-A"]).unwrap();
        commit(&f, "dir becomes file");
        f.git(&["update-index", "--chmod=+x", "a"]).unwrap();
        commit(&f, "only the mode");
        f.write_file("l.txt", "target\n").unwrap();
        let id = f.git(&["hash-object", "-w", "l.txt"]).unwrap();
        f.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{},link", id.trim()),
        ])
        .unwrap();
        let head = f.oid("HEAD").unwrap();
        f.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},sub", head.trim()),
        ])
        .unwrap();
        commit(&f, "link and gitlink");
        f.git(&["rm", "-q", "--cached", "link", "sub"]).unwrap();
        f.write_file("deep/one/other/Leaf2.TXT", "NEEDLE two\n")
            .unwrap();
        f.git(&["rm", "-q", "-r", "deep/one/two"]).unwrap();
        f.git(&["add", "-A"]).unwrap();
        commit(&f, "nested removal and addition");
        f
    }

    #[test]
    fn the_hand_walked_diff_finds_what_gix_does() {
        let mut hits = 0;
        for f in [tricky(), test_fixtures::linear(40).unwrap()] {
            let repo = RepoHandle::open(f.path()).unwrap();
            let ids: Vec<gix::ObjectId> = f
                .git(&["rev-list", "--all"])
                .unwrap()
                .lines()
                .map(|line| gix::ObjectId::from_hex(line.trim().as_bytes()).unwrap())
                .collect();
            let needles = [
                "foo",
                "a",
                "a.b",
                "x.txt",
                "a.b/",
                "a/",
                "deep/",
                "deep/one/two/leaf",
                "leaf",
                "needle",
                "link",
                "sub",
                "file1",
                "content 1",
                "dotted",
                "now a dir",
                "zzz",
            ];
            for needle in needles {
                for (name, content) in [(true, false), (false, true), (true, true)] {
                    let text = TextMatch {
                        needle: needle.to_owned(),
                        fields: TextFields {
                            name,
                            content,
                            ..TextFields::default()
                        },
                        refs: HashMap::new(),
                        notes: HashMap::new(),
                    };
                    for id in &ids {
                        let fast = text.matches_changes(&repo.repo, *id);
                        hits += usize::from(fast);
                        assert_eq!(
                            fast,
                            text.matches_changes_gix(&repo.repo, *id),
                            "{needle} name={name} content={content} at {id}"
                        );
                    }
                }
            }
        }
        assert!(
            hits > 30,
            "{hits} matches: the shapes did not exercise the walk"
        );
    }
}
