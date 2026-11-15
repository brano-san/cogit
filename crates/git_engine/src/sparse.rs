//! Sparse checkout (F-955): the current set, folders of HEAD to choose from, what a change
//! does to the working tree, and the change itself.

use crate::{FileStatus, GitError, RepoHandle, Result};
use gix::bstr::{BStr, ByteSlice as _};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SparseState {
    pub enabled: bool,
    /// Cone mode: `set` lists folders. Otherwise it holds gitignore-style patterns. While
    /// sparse checkout is off it says what Enable starts with: cone.
    pub cone: bool,
    pub set: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SparseChange {
    /// `git sparse-checkout set`; also turns sparse checkout on.
    Set {
        cone: bool,
        set: Vec<String>,
    },
    Disable,
}

impl SparseChange {
    #[must_use]
    pub fn restoring(state: &SparseState) -> Self {
        if state.enabled {
            Self::Set {
                cone: state.cone,
                set: state.set.clone(),
            }
        } else {
            Self::Disable
        }
    }
}

/// What a change does to the working tree, counted over the index (`git ls-files`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SparsePreview {
    pub tracked: u32,
    pub present: u32,
    pub appearing: u32,
    pub removed: u32,
    /// Files that would leave the working tree with local changes: the change is refused.
    pub blocked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SparseFolder {
    pub path: String,
    pub folders: bool,
}

impl RepoHandle {
    /// Read from git, not the handle's config snapshot: it is asked again right after a change.
    pub fn sparse_state(&self) -> Result<SparseState> {
        if !self.config_flag("core.sparseCheckout")? {
            return Ok(SparseState {
                enabled: false,
                cone: true,
                set: Vec::new(),
            });
        }
        // Unset means non-cone to git; `sparse-checkout set` writes it either way.
        let cone = self.config_flag("core.sparseCheckoutCone")?;
        let listed = self.read_git(&["-c", "core.quotePath=false", "sparse-checkout", "list"])?;
        let set = listed
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                if cone {
                    crate::blame_origins::unquote_path(line)
                } else {
                    line.to_owned()
                }
            })
            .collect();
        Ok(SparseState {
            enabled: true,
            cone,
            set,
        })
    }

    /// The folders right inside `dir` (`""`: the top) in HEAD's tree; none on an unborn branch.
    pub fn sparse_folders(&self, dir: &str) -> Result<Vec<SparseFolder>> {
        let Ok(commit) = self.repo.head_commit() else {
            return Ok(Vec::new());
        };
        let unreadable = |err: &dyn std::fmt::Display| {
            GitError::Internal(format!("cannot read the tree of HEAD: {err}"))
        };
        let mut tree = commit.tree().map_err(|err| unreadable(&err))?;
        let dir = dir.trim_matches('/');
        if !dir.is_empty() {
            let Some(entry) = tree
                .lookup_entry_by_path(dir)
                .map_err(|err| unreadable(&err))?
            else {
                return Ok(Vec::new());
            };
            if !entry.mode().is_tree() {
                return Ok(Vec::new());
            }
            tree = entry.object().map_err(|err| unreadable(&err))?.into_tree();
        }
        let subtrees = |tree: &gix::Tree<'_>| -> Vec<(String, gix::ObjectId)> {
            tree.iter()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry.mode().is_tree())
                .map(|entry| {
                    (
                        entry.filename().to_str_lossy().into_owned(),
                        entry.oid().to_owned(),
                    )
                })
                .collect()
        };
        let mut folders = Vec::new();
        for (name, oid) in subtrees(&tree) {
            let inner = self.repo.find_tree(oid).map_err(|err| unreadable(&err))?;
            folders.push(SparseFolder {
                path: if dir.is_empty() {
                    name
                } else {
                    format!("{dir}/{name}")
                },
                folders: !subtrees(&inner).is_empty(),
            });
        }
        Ok(folders)
    }

    pub fn sparse_preview(&self, change: &SparseChange) -> Result<SparsePreview> {
        let listed = self.read_git(&["-c", "core.quotePath=false", "ls-files", "-z", "-t"])?;
        let keeps = Keeps::new(change, self.ignore_case());
        let mut preview = SparsePreview::default();
        let mut leaving = Vec::new();
        let mut last: Option<&str> = None;
        for item in listed.split('\0').filter(|item| item.len() > 2) {
            let (tag, path) = item.split_at(2);
            if last.replace(path) == Some(path) {
                continue;
            }
            let here = !tag.starts_with('S');
            let kept = keeps.keeps(path);
            preview.tracked += 1;
            preview.present += u32::from(kept);
            preview.appearing += u32::from(kept && !here);
            if here && !kept {
                preview.removed += 1;
                leaving.push(path);
            }
        }
        if !leaving.is_empty() {
            let changed = self.locally_changed()?;
            preview.blocked = leaving
                .into_iter()
                .filter(|path| changed.contains(*path))
                .map(str::to_owned)
                .collect();
        }
        Ok(preview)
    }

    /// Refused while a file leaving the working tree has local changes: git would leave it
    /// behind anyway, outside the set and easy to forget.
    pub fn sparse_apply(&self, change: &SparseChange) -> Result<()> {
        match change {
            SparseChange::Disable => self
                .run_git_stoppable(&["sparse-checkout", "disable"], None)
                .map(drop),
            SparseChange::Set { cone, set } => {
                let set = normalized(*cone, set);
                let change = SparseChange::Set {
                    cone: *cone,
                    set: set.clone(),
                };
                refuse_blocked(&self.sparse_preview(&change)?)?;
                let mode = if *cone { "--cone" } else { "--no-cone" };
                let mut input = set
                    .iter()
                    .map(|line| if *cone { quoted(line) } else { line.clone() })
                    .collect::<Vec<_>>()
                    .join("\n");
                input.push('\n');
                self.run_git_stoppable(
                    &["sparse-checkout", "set", mode, "--stdin"],
                    Some(input.as_bytes()),
                )
                .map(drop)
            }
        }
    }

    fn config_flag(&self, key: &str) -> Result<bool> {
        let value = self.read_git(&["config", "--type=bool", "--default=false", "--get", key])?;
        Ok(value.trim() == "true")
    }

    fn ignore_case(&self) -> bool {
        self.repo.config_snapshot().boolean("core.ignoreCase") == Some(true)
    }

    fn locally_changed(&self) -> Result<HashSet<String>> {
        let files = self.worktree_files()?;
        Ok(files
            .staged
            .into_iter()
            .chain(files.unstaged)
            .filter(|file| {
                !matches!(
                    file.status,
                    FileStatus::Untracked | FileStatus::Ignored | FileStatus::Sparse
                )
            })
            .map(|file| file.path)
            .collect())
    }

    /// The set's size for the repository row: folders in cone mode, else patterns. From the
    /// config snapshot and the patterns file, so a row costs no git process.
    #[must_use]
    pub fn sparse_size(&self) -> Option<u32> {
        if !self.sparse_checkout() {
            return None;
        }
        let cone = self
            .repo
            .config_snapshot()
            .boolean("core.sparseCheckoutCone")
            .unwrap_or(false);
        let file = self.repo.git_dir().join("info").join("sparse-checkout");
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        let size = if cone {
            cone_folder_count(&lines)
        } else {
            lines.len()
        };
        Some(u32::try_from(size).unwrap_or(u32::MAX))
    }
}

/// A cone file names each folder of the set as `/dir/`; a folder only on the way to one is
/// followed by `!/dir/*/`.
fn cone_folder_count(lines: &[&str]) -> usize {
    let passed: HashSet<&str> = lines
        .iter()
        .filter_map(|line| line.strip_prefix('!')?.strip_suffix("/*/"))
        .collect();
    lines
        .iter()
        .filter(|line| line.len() > 2 && line.starts_with('/') && line.ends_with('/'))
        .filter(|line| !passed.contains(line.trim_end_matches('/')))
        .count()
}

fn refuse_blocked(preview: &SparsePreview) -> Result<()> {
    if preview.blocked.is_empty() {
        return Ok(());
    }
    const SHOWN: usize = 10;
    let mut named = preview
        .blocked
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if preview.blocked.len() > SHOWN {
        named.push_str(&format!("\nand {} more", preview.blocked.len() - SHOWN));
    }
    Err(GitError::InvalidState(format!(
        "These files would leave the working tree with local changes. Commit, stash or discard them first:\n{named}"
    )))
}

fn normalized(cone: bool, set: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    set.iter()
        .map(|line| {
            if cone {
                line.trim().trim_matches('/').to_owned()
            } else {
                line.trim_end().to_owned()
            }
        })
        .filter(|line| !line.is_empty() && seen.insert(line.clone()))
        .collect()
}

/// `sparse-checkout set --stdin` unquotes a line that starts with `"` in cone mode.
fn quoted(path: &str) -> String {
    if !path.contains(['"', '\\', '\n', '\t']) {
        return path.to_owned();
    }
    let mut out = String::from("\"");
    for ch in path.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

enum Keeps {
    All,
    Cone(Vec<String>),
    Patterns(Box<gix::ignore::Search>, gix::glob::pattern::Case),
}

impl Keeps {
    fn new(change: &SparseChange, ignore_case: bool) -> Self {
        match change {
            SparseChange::Disable => Self::All,
            SparseChange::Set { cone: true, set } => Self::Cone(normalized(true, set)),
            SparseChange::Set { cone: false, set } => {
                let mut search = gix::ignore::Search::default();
                search.add_patterns_buffer(
                    normalized(false, set).join("\n").as_bytes(),
                    "sparse-checkout",
                    None,
                    gix::ignore::search::Ignore::default(),
                );
                let case = if ignore_case {
                    gix::glob::pattern::Case::Fold
                } else {
                    gix::glob::pattern::Case::Sensitive
                };
                Self::Patterns(Box::new(search), case)
            }
        }
    }

    fn keeps(&self, path: &str) -> bool {
        match self {
            Self::All => true,
            Self::Cone(folders) => in_cone(path, folders),
            Self::Patterns(search, case) => in_patterns(search, path, *case),
        }
    }
}

/// Git's cone rule: files at the top, everything under a folder of the set, and the files
/// right inside each folder on the way to one.
pub(crate) fn in_cone(path: &str, folders: &[String]) -> bool {
    let Some((parent, _)) = path.rsplit_once('/') else {
        return true;
    };
    let under = |inner: &str, outer: &str| {
        inner.len() > outer.len()
            && inner.starts_with(outer)
            && inner.as_bytes().get(outer.len()) == Some(&b'/')
    };
    folders
        .iter()
        .any(|folder| under(path, folder) || folder == parent || under(folder, parent))
}

/// Git's non-cone rule: the last pattern matching the file decides; with none, the last
/// one matching its nearest folder; with none at all, it is out.
fn in_patterns(search: &gix::ignore::Search, path: &str, case: gix::glob::pattern::Case) -> bool {
    let mut current = path;
    let mut is_dir = false;
    loop {
        let relative: &BStr = current.as_bytes().as_bstr();
        if let Some(found) = search.pattern_matching_relative_path(relative, Some(is_dir), case) {
            return !found.pattern.is_negative();
        }
        match current.rsplit_once('/') {
            Some((parent, _)) => {
                current = parent;
                is_dir = true;
            }
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(folders: &[&str]) -> Vec<String> {
        folders.iter().map(|each| (*each).to_owned()).collect()
    }

    #[test]
    fn the_cone_holds_top_files_its_folders_and_files_on_the_way() {
        let folders = set(&["a/b", "c"]);
        assert!(in_cone("README.md", &folders));
        assert!(in_cone("a/b/deep/x.rs", &folders));
        assert!(
            in_cone("a/top.txt", &folders),
            "a file right inside a parent"
        );
        assert!(in_cone("c/x", &folders));
        assert!(!in_cone("a/other/x", &folders));
        assert!(!in_cone("ab/x", &folders));
        assert!(!in_cone("a/bc/x", &folders));
        assert!(!in_cone("d/x", &folders));
    }

    #[test]
    fn an_empty_cone_keeps_only_the_top() {
        assert!(in_cone("x", &[]));
        assert!(!in_cone("a/x", &[]));
    }

    #[test]
    fn patterns_follow_gitignore_rules_with_the_last_match_deciding() {
        let keeps = Keeps::new(
            &SparseChange::Set {
                cone: false,
                set: set(&["/*", "!/*/", "/docs/", "!/docs/draft/", "*.md"]),
            },
            false,
        );
        assert!(keeps.keeps("top.txt"));
        assert!(keeps.keeps("docs/a.txt"));
        assert!(keeps.keeps("docs/deep/a.txt"));
        assert!(!keeps.keeps("docs/draft/a.txt"));
        assert!(keeps.keeps("src/notes.md"));
        assert!(!keeps.keeps("src/main.rs"));
    }

    #[test]
    fn odd_folder_names_are_quoted_for_stdin() {
        assert_eq!(quoted("plain dir"), "plain dir");
        assert_eq!(quoted("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn a_cone_file_counts_the_folders_of_its_set() {
        let lines = ["/*", "!/*/", "/a/", "!/a/*/", "/a/b/", "/c/"];
        assert_eq!(cone_folder_count(&lines), 2);
        assert_eq!(cone_folder_count(&["/*", "!/*/"]), 0);
    }

    #[test]
    fn cone_folders_lose_their_slashes_and_repeats() {
        assert_eq!(
            normalized(true, &set(&["/a/", "a", " ", "b/"])),
            set(&["a", "b"])
        );
    }
}
