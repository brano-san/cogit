use crate::{CommitQuery, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FoundKind {
    Branch,
    Tag,
    Commit,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub kind: FoundKind,
    pub label: String,
    pub detail: String,
    pub oid: String,
}

impl RepoHandle {
    /// Refs first, then commits, then files: a name the user types is far more likely to
    /// be a branch than a path that happens to contain it.
    pub fn find(&self, query: &str, limit: usize) -> Result<Vec<Found>> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }

        let mut found = Vec::new();
        for branch in self.branches()? {
            if branch.name.to_lowercase().contains(&needle) {
                found.push(Found {
                    kind: FoundKind::Branch,
                    label: branch.name,
                    detail: branch.full_name,
                    oid: branch.oid,
                });
            }
        }
        for tag in self.tags()? {
            if tag.name.to_lowercase().contains(&needle) {
                found.push(Found {
                    kind: FoundKind::Tag,
                    label: tag.name,
                    detail: tag.full_name,
                    oid: tag.oid,
                });
            }
        }

        let looks_like_hash = needle.len() >= 4 && needle.chars().all(|c| c.is_ascii_hexdigit());
        let search = CommitQuery {
            message: (!looks_like_hash).then(|| needle.clone()),
            oid_prefix: looks_like_hash.then(|| needle.clone()),
            ..CommitQuery::default()
        };
        let mut commits = 0;
        self.search_commits(&search, 50, |chunk| {
            for row in chunk {
                found.push(Found {
                    kind: FoundKind::Commit,
                    label: row.summary,
                    detail: row.author_name,
                    oid: row.oid,
                });
                commits += 1;
            }
            commits < limit
        })?;

        for path in self.head_paths(&needle, limit)? {
            found.push(Found {
                kind: FoundKind::File,
                label: path,
                detail: "in the current tree".to_owned(),
                oid: String::new(),
            });
        }

        found.truncate(limit);
        Ok(found)
    }

    fn head_paths(&self, needle: &str, limit: usize) -> Result<Vec<String>> {
        let Ok(head) = self.repo.head_commit() else {
            return Ok(Vec::new());
        };
        let Ok(tree) = head.tree() else {
            return Ok(Vec::new());
        };

        let mut recorder = gix::traverse::tree::Recorder::default();
        if tree.traverse().breadthfirst(&mut recorder).is_err() {
            return Ok(Vec::new());
        }

        let mut paths = Vec::new();
        for entry in recorder.records {
            if paths.len() >= limit {
                break;
            }
            let path = entry.filepath.to_string();
            if path.to_lowercase().contains(needle) {
                paths.push(path);
            }
        }
        Ok(paths)
    }
}
