//! What the Conflict Solver needs besides the three sides: which operation stopped (to
//! name "ours" and "theirs"), the merge tool the user set up in git, and marking resolved.

use crate::{Head, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictOperation {
    Merge,
    CherryPick,
    Revert,
    Rebase,
    StashApply,
    Unknown,
}

/// Names for the two sides, as the solver's panel headers show them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictContext {
    pub operation: ConflictOperation,
    pub ours: String,
    pub theirs: String,
    pub ours_side: SideName,
    pub theirs_side: SideName,
}

/// A side as the solver's pane header names it: its part in the operation and its commit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SideName {
    pub role: String,
    /// Short hash of the commit the side comes from.
    pub oid: Option<String>,
    /// Its subject, for the header's tooltip.
    pub subject: Option<String>,
}

fn merged_role(message: Option<&str>) -> String {
    let line = message
        .and_then(|text| text.lines().next())
        .unwrap_or_default();
    if line.starts_with("Merge commit") {
        return "Merged commit".to_owned();
    }
    match merge_target(line) {
        Some(name) if line.starts_with("Merge tag") => format!("Merged tag '{name}'"),
        Some(name) => format!("Merged branch '{name}'"),
        None => "Merged commit".to_owned(),
    }
}

/// `merge.tool` and what `mergetool.<tool>.cmd|path` say about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeToolConfig {
    pub tool: Option<String>,
    pub cmd: Option<String>,
    pub path: Option<String>,
}

const SHORT: usize = 7;
const STASH_LABEL: &str = "Stashed changes";

fn short(oid: &str) -> String {
    oid.chars().take(SHORT).collect()
}

/// `Merge branch 'feature' into main` names `feature`; the same for remote-tracking
/// branches, tags and commits. Octopus merges name the first.
#[must_use]
pub fn merge_target(message: &str) -> Option<String> {
    let line = message.lines().next()?;
    let open = line.find('\'')?;
    let rest = line.get(open + 1..)?;
    let close = rest.find('\'')?;
    let name = rest.get(..close)?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// The labels git wrote after the first `<<<<<<<` and `>>>>>>>` of a file: a stash apply
/// leaves no state file, but its markers say `Stashed changes`.
#[must_use]
pub fn marker_labels(text: &str) -> (Option<String>, Option<String>) {
    let label = |prefix: &str, line: &str| {
        line.strip_prefix(prefix)
            .map(|rest| rest.trim().to_owned())
            .filter(|rest| !rest.is_empty())
    };
    let mut ours = None;
    let mut theirs = None;
    for line in text.lines() {
        if ours.is_none() {
            ours = label("<<<<<<<", line);
        }
        if theirs.is_none() {
            theirs = label(">>>>>>>", line);
        }
    }
    (ours, theirs)
}

impl RepoHandle {
    fn git_file(&self, name: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.git_dir().join(name)).ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    fn subject_of(&self, oid: &str) -> Option<String> {
        let line = self
            .read_git(&[
                "log",
                "--no-show-signature",
                "--encoding=UTF-8",
                "-1",
                "--format=%s",
                oid,
            ])
            .ok()?;
        let line = line.trim();
        (!line.is_empty()).then(|| line.to_owned())
    }

    fn commit_label(&self, oid: &str) -> String {
        match self.subject_of(oid) {
            Some(subject) => format!("{} {subject}", short(oid)),
            None => short(oid),
        }
    }

    fn side(&self, role: impl Into<String>, oid: Option<&str>) -> SideName {
        SideName {
            role: role.into(),
            oid: oid.map(short),
            subject: oid.and_then(|oid| self.subject_of(oid)),
        }
    }

    fn current_side(&self) -> SideName {
        match self.head() {
            Ok(Head::Branch { name, oid }) => {
                self.side(format!("Current branch '{name}'"), Some(&oid))
            }
            Ok(Head::Unborn { name }) => self.side(format!("Current branch '{name}'"), None),
            Ok(Head::Detached { oid }) => self.side("Current commit", Some(&oid)),
            Err(err) => {
                tracing::error!(error = ?err, context = "naming ours for the conflict solver");
                self.side("ours", None)
            }
        }
    }

    fn branch_label(&self) -> String {
        match self.head() {
            Ok(Head::Branch { name, .. } | Head::Unborn { name }) => name,
            Ok(Head::Detached { oid }) => format!("HEAD ({})", short(&oid)),
            Err(err) => {
                tracing::error!(error = ?err, context = "naming ours for the conflict solver");
                "HEAD".to_owned()
            }
        }
    }

    /// A ref that points at `oid`, for a rebase that left HEAD detached on it.
    fn name_of_commit(&self, oid: &str) -> String {
        let named = self
            .read_git(&[
                "for-each-ref",
                "--points-at",
                oid,
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ])
            .ok()
            .and_then(|out| out.lines().next().map(str::to_owned));
        named.unwrap_or_else(|| short(oid))
    }

    /// Never fails: a label git leaves no trace of is `theirs`, and the sides are the same.
    #[must_use]
    pub fn conflict_context(&self, path: &str) -> ConflictContext {
        let git_dir = self.git_dir();
        let exists = |name: &str| git_dir.join(name).exists();

        if exists("rebase-merge") || exists("rebase-apply") {
            let backend = if exists("rebase-merge") {
                "rebase-merge"
            } else {
                "rebase-apply"
            };
            let onto = self.git_file(&format!("{backend}/onto"));
            let ours = onto
                .as_deref()
                .map_or_else(|| self.branch_label(), |onto| self.name_of_commit(onto));
            let applied = self
                .git_file("REBASE_HEAD")
                .or_else(|| self.git_file(&format!("{backend}/stopped-sha")));
            let branch = self
                .git_file(&format!("{backend}/head-name"))
                .filter(|name| name != "detached HEAD")
                .map(|name| name.trim_start_matches("refs/heads/").to_owned());
            let theirs = match (&applied, &branch) {
                (Some(oid), _) => self.commit_label(oid),
                (None, Some(name)) => name.clone(),
                (None, None) => "theirs".to_owned(),
            };
            let rebased = branch.map_or_else(
                || "Rebased commit".to_owned(),
                |name| format!("Rebased branch '{name}'"),
            );
            return ConflictContext {
                operation: ConflictOperation::Rebase,
                ours,
                theirs,
                ours_side: self.side("Rebase target", onto.as_deref()),
                theirs_side: self.side(rebased, applied.as_deref()),
            };
        }
        if let Some(oid) = self.git_file("MERGE_HEAD") {
            let message = self.git_file("MERGE_MSG");
            let theirs = message
                .as_deref()
                .and_then(merge_target)
                .unwrap_or_else(|| short(&oid));
            return ConflictContext {
                operation: ConflictOperation::Merge,
                ours: self.branch_label(),
                theirs,
                ours_side: self.current_side(),
                theirs_side: self.side(merged_role(message.as_deref()), Some(&oid)),
            };
        }
        if let Some(oid) = self.git_file("CHERRY_PICK_HEAD") {
            return ConflictContext {
                operation: ConflictOperation::CherryPick,
                ours: self.branch_label(),
                theirs: self.commit_label(&oid),
                ours_side: self.current_side(),
                theirs_side: self.side("Cherry-picked commit", Some(&oid)),
            };
        }
        if let Some(oid) = self.git_file("REVERT_HEAD") {
            return ConflictContext {
                operation: ConflictOperation::Revert,
                ours: self.branch_label(),
                theirs: format!("Revert of {}", self.commit_label(&oid)),
                ours_side: self.current_side(),
                theirs_side: self.side("Reverted commit", Some(&oid)),
            };
        }

        let marked = std::fs::read(self.root().join(path))
            .map(|bytes| marker_labels(&String::from_utf8_lossy(&bytes)))
            .unwrap_or_default();
        let theirs = marked.1.unwrap_or_else(|| "theirs".to_owned());
        if theirs == STASH_LABEL {
            return ConflictContext {
                operation: ConflictOperation::StashApply,
                ours: self.branch_label(),
                theirs,
                ours_side: self.side("Working tree before apply", None),
                theirs_side: self.side("Stash", None),
            };
        }
        ConflictContext {
            operation: ConflictOperation::Unknown,
            ours: self.branch_label(),
            theirs_side: self.side(theirs.clone(), None),
            theirs,
            ours_side: self.current_side(),
        }
    }

    /// `git config --list -z` always succeeds, where `--get` of a key nobody set exits 1
    /// and would reach the journal as a failed command.
    pub fn merge_tool_config(&self) -> Result<MergeToolConfig> {
        let listing = self.read_git(&["config", "--list", "-z"])?;
        Ok(merge_tool_from_listing(&listing))
    }

    /// Git added: the working file has no conflict markers left and is taken as it is.
    pub fn mark_resolved(&self, path: &str) -> Result<()> {
        if !self.conflicted_paths()?.iter().any(|p| p == path) {
            return Err(crate::GitError::InvalidState(format!(
                "{path} is not conflicted"
            )));
        }
        self.run_git_literal(&["add", "--", path]).map(drop)
    }
}

/// The last value wins, as git reads a key set at several levels.
#[must_use]
pub fn merge_tool_from_listing(listing: &str) -> MergeToolConfig {
    let mut config = MergeToolConfig::default();
    let mut commands: Vec<(String, String)> = Vec::new();
    let mut paths: Vec<(String, String)> = Vec::new();
    for entry in listing.split('\0').filter(|entry| !entry.is_empty()) {
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        let lowered = key.to_ascii_lowercase();
        if lowered == "merge.tool" {
            config.tool = Some(value.to_owned());
        } else if let Some(rest) = lowered.strip_prefix("mergetool.") {
            if let Some(tool) = rest.strip_suffix(".cmd") {
                commands.push((tool.to_owned(), value.to_owned()));
            } else if let Some(tool) = rest.strip_suffix(".path") {
                paths.push((tool.to_owned(), value.to_owned()));
            }
        }
    }
    if let Some(tool) = config.tool.as_deref().map(str::to_ascii_lowercase) {
        let pick = |all: &[(String, String)]| {
            all.iter()
                .rev()
                .find(|(name, _)| *name == tool)
                .map(|(_, value)| value.clone())
        };
        config.cmd = pick(&commands);
        config.path = pick(&paths);
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_message_names_the_branch_in_quotes() {
        assert_eq!(
            merge_target("Merge branch 'feature/x' into main\n"),
            Some("feature/x".to_owned())
        );
        assert_eq!(
            merge_target("Merge remote-tracking branch 'origin/dev'"),
            Some("origin/dev".to_owned())
        );
        assert_eq!(merge_target("Merge tag 'v1.2'"), Some("v1.2".to_owned()));
        assert_eq!(merge_target("Merge made by hand"), None);
        assert_eq!(merge_target(""), None);
    }

    #[test]
    fn marker_labels_are_read_from_the_first_markers_of_the_file() {
        let text = "a\n<<<<<<< Updated upstream\nx\n=======\ny\n>>>>>>> Stashed changes\nb\n<<<<<<< later\n";
        assert_eq!(
            marker_labels(text),
            (
                Some("Updated upstream".to_owned()),
                Some("Stashed changes".to_owned())
            )
        );
        assert_eq!(marker_labels("plain\n"), (None, None));
    }

    #[test]
    fn marker_labels_survive_crlf() {
        let (ours, theirs) =
            marker_labels("<<<<<<< HEAD\r\nx\r\n=======\r\ny\r\n>>>>>>> topic\r\n");
        assert_eq!(ours.as_deref(), Some("HEAD"));
        assert_eq!(theirs.as_deref(), Some("topic"));
    }

    #[test]
    fn the_configured_merge_tool_comes_with_its_own_command_and_path() {
        let listing = "user.name\nme\0merge.tool\nmeld\0mergetool.meld.path\nC:/Meld/Meld.exe\0mergetool.other.cmd\nx\0";
        let config = merge_tool_from_listing(listing);
        assert_eq!(config.tool.as_deref(), Some("meld"));
        assert_eq!(config.path.as_deref(), Some("C:/Meld/Meld.exe"));
        assert_eq!(config.cmd, None);
    }

    #[test]
    fn no_merge_tool_is_an_empty_config() {
        assert_eq!(
            merge_tool_from_listing("user.name\nme\0"),
            MergeToolConfig::default()
        );
    }

    #[test]
    fn the_last_setting_wins() {
        let listing = "merge.tool\na\0merge.tool\nb\0mergetool.b.cmd\none\0mergetool.b.cmd\ntwo\0";
        let config = merge_tool_from_listing(listing);
        assert_eq!(config.tool.as_deref(), Some("b"));
        assert_eq!(config.cmd.as_deref(), Some("two"));
    }
}
