use crate::line_history::{LogHeader, path_of};
use crate::{CommitQuery, GitError, RepoHandle, Result};
use serde::Serialize;
use std::collections::HashSet;

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

        // "added" or "cafe" is a word as often as a hash: both are looked for.
        let looks_like_hash = needle.len() >= 4 && needle.chars().all(|c| c.is_ascii_hexdigit());
        let by_prefix = if looks_like_hash {
            self.commits_by_prefix(&needle, limit)
        } else {
            Vec::new()
        };
        let mut listed: HashSet<String> = by_prefix.iter().map(|item| item.oid.clone()).collect();
        let mut commits = by_prefix.len();
        found.extend(by_prefix);
        if commits < limit {
            let search = CommitQuery {
                message: Some(needle.clone()),
                ..CommitQuery::default()
            };
            self.search_commits(&search, 50, |chunk| {
                for row in chunk {
                    if !listed.insert(row.oid.clone()) {
                        continue;
                    }
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
        }

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

    /// Commits whose id starts with `hex`, read from the pack indexes instead of a walk that
    /// reads every commit's text. Unreachable commits are among them, as `git show` finds them.
    fn commits_by_prefix(&self, hex: &str, limit: usize) -> Vec<Found> {
        let Ok(prefix) = gix::hash::Prefix::from_hex(hex) else {
            return Vec::new();
        };
        let mut candidates = HashSet::new();
        if let Err(err) = self
            .repo
            .objects
            .lookup_prefix(prefix, Some(&mut candidates))
        {
            tracing::error!(error = ?err, context = "find: looking up an object id prefix");
            return Vec::new();
        }
        let mut ids: Vec<gix::ObjectId> = candidates.into_iter().collect();
        ids.sort_unstable();
        let mailmap = self.mailmap();
        ids.into_iter()
            .filter(|id| {
                self.repo
                    .find_header(*id)
                    .is_ok_and(|header| header.kind() == gix::object::Kind::Commit)
            })
            .take(limit)
            .filter_map(|id| {
                let oid = id.to_string();
                let text = self.commit_text(&oid, &mailmap).ok()?;
                Some(Found {
                    kind: FoundKind::Commit,
                    label: text.summary,
                    detail: text.author_name,
                    oid,
                })
            })
            .collect()
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
            // Folders have nothing to diff; a gitlink stays, as `tree_files` keeps it.
            if entry.mode.is_tree() {
                continue;
            }
            let path = entry.filepath.to_string();
            if path.to_lowercase().contains(needle) {
                paths.push(path);
            }
        }
        Ok(paths)
    }
}

/// One edit to the fragment under investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationStep {
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    /// The path the file had at this commit, which a rename changes under the range.
    pub path: String,
    /// The unified diff of this edit, restricted to the range.
    pub diff: String,
}

/// Starts a record. A diff line can never begin with it: diff bodies start with `diff`,
/// `---`, `+++`, `@@`, a space, `+`, `-` or a backslash.
const STEP: &str = "<<cogit-step>>";

impl RepoHandle {
    /// Every commit that changed lines `from..=to` of `path`, newest first, each with the
    /// diff of that one edit.
    ///
    /// `git log -L` traces the range through edits and follows it across renames, which is
    /// why this one read goes through the CLI rather than `gix` (R-104).
    pub fn investigate(
        &self,
        path: &str,
        from: u32,
        to: u32,
        limit: usize,
    ) -> Result<Vec<InvestigationStep>> {
        if from == 0 {
            return Err(GitError::InvalidState("line numbers start at 1".to_owned()));
        }
        if to < from {
            return Err(GitError::InvalidState(format!(
                "range {from},{to} ends before it starts"
            )));
        }

        let range = format!("{from},{to}:{path}");
        let count = format!("-{}", limit.clamp(1, 1000));
        let output = self.read_git(&[
            "log",
            "-L",
            &range,
            &count,
            "--no-color",
            &format!("--format={STEP}%H%x09%aN%x09%aE%x09%at%x09%s"),
        ])?;

        Ok(parse_investigation(&output))
    }
}

fn parse_investigation(stdout: &str) -> Vec<InvestigationStep> {
    let mut steps: Vec<InvestigationStep> = Vec::new();

    for line in stdout.lines() {
        if let Some(header) = line.strip_prefix(STEP) {
            if let Some(step) = header_to_step(header) {
                steps.push(step);
            }
            continue;
        }
        let Some(step) = steps.last_mut() else {
            continue;
        };
        if step.path.is_empty()
            && let Some(path) = path_of(line)
        {
            step.path = path;
        }
        step.diff.push_str(line);
        step.diff.push('\n');
    }

    for step in &mut steps {
        let trimmed = step.diff.trim_matches('\n').to_owned();
        step.diff = trimmed;
    }
    steps
}

fn header_to_step(header: &str) -> Option<InvestigationStep> {
    let LogHeader {
        oid,
        author,
        email,
        timestamp,
        summary,
    } = LogHeader::parse(header)?;

    Some(InvestigationStep {
        oid,
        summary,
        author,
        email,
        timestamp,
        path: String::new(),
        diff: String::new(),
    })
}
