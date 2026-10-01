//! What the Branches panel needs beyond the ref list: how tags group, how old each tip is.

use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

const TAG_SEPARATOR_KEY: &str = "cogit.tagGroupSeparator";
const DEFAULT_TAG_SEPARATOR: &str = "/";

/// When a ref's tip was made, for sorting Branches by date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RefDate {
    pub full_name: String,
    /// Unix seconds: the tagger's for an annotated tag, the committer's otherwise.
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

/// A ref outside branches and tags: `refs/pull/1/head`, `refs/changes/…`, a tool's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OtherRef {
    pub full_name: String,
    pub oid: String,
}

/// Shown by their own groups.
const LISTED_ELSEWHERE: &[&str] = &["refs/heads/", "refs/remotes/", "refs/tags/", "refs/stash"];

/// Not under `refs/`: where the last reset, rebase or merge started from. `FETCH_HEAD` is a
/// list of lines, not a ref; `AUTO_MERGE` is a tree.
const PSEUDO_REFS: [&str; 5] = [
    "ORIG_HEAD",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "REBASE_HEAD",
];

impl RepoHandle {
    /// Peeled to commits; a ref to a tree or a blob has no history to show.
    pub fn other_refs(&self) -> Result<Vec<OtherRef>> {
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        let refs = platform
            .prefixed("refs/")
            .map_err(|err| GitError::Internal(format!("cannot list references: {err}")))?;
        let mut found = Vec::new();
        for mut reference in refs.flatten() {
            let full_name = reference.name().as_bstr().to_string();
            if LISTED_ELSEWHERE
                .iter()
                .any(|prefix| full_name.starts_with(prefix))
            {
                continue;
            }
            let Ok(id) = reference.peel_to_id() else {
                continue;
            };
            if self.repo.find_commit(id.detach()).is_err() {
                continue;
            }
            found.push(OtherRef {
                full_name,
                oid: id.to_string(),
            });
        }
        found.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        for name in PSEUDO_REFS {
            let Ok(mut reference) = self.repo.find_reference(name) else {
                continue;
            };
            if let Ok(id) = reference.peel_to_id()
                && self.repo.find_commit(id.detach()).is_ok()
            {
                found.push(OtherRef {
                    full_name: name.to_owned(),
                    oid: id.to_string(),
                });
            }
        }
        Ok(found)
    }

    /// The commit `ORIG_HEAD` names, if there is one: what the last reset, rebase or merge left.
    #[must_use]
    pub fn orig_head(&self) -> Option<String> {
        let mut reference = self.repo.find_reference("ORIG_HEAD").ok()?;
        let id = reference.peel_to_id().ok()?.detach();
        self.repo.find_commit(id).ok()?;
        Some(id.to_string())
    }

    /// Written by Repository ▸ Settings ▸ Tag-Grouping; an empty value turns folders off.
    #[must_use]
    pub fn tag_group_separator(&self) -> String {
        self.repo
            .config_snapshot()
            .string(TAG_SEPARATOR_KEY)
            .map_or_else(
                || DEFAULT_TAG_SEPARATOR.to_owned(),
                |value| value.to_string(),
            )
    }

    /// Read only when the user sorts by date: it opens one object per ref.
    pub fn ref_dates(&self) -> Result<Vec<RefDate>> {
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        let refs = platform
            .all()
            .map_err(|err| GitError::Internal(format!("cannot list references: {err}")))?;

        let mut dates = Vec::new();
        for reference in refs.flatten() {
            let full_name = reference.name().as_bstr().to_string();
            if !["refs/heads/", "refs/remotes/", "refs/tags/"]
                .iter()
                .any(|prefix| full_name.starts_with(prefix))
            {
                continue;
            }
            let Some(id) = reference.try_id() else {
                continue;
            };
            if let Some(timestamp) = self.date_of(id.detach()) {
                dates.push(RefDate {
                    full_name,
                    timestamp,
                });
            }
        }
        Ok(dates)
    }

    fn date_of(&self, id: gix::ObjectId) -> Option<i64> {
        let object = self.repo.find_object(id).ok()?;
        match object.kind {
            gix::object::Kind::Commit => object
                .try_into_commit()
                .ok()?
                .time()
                .ok()
                .map(|t| t.seconds),
            gix::object::Kind::Tag => {
                let tag = object.try_into_tag().ok()?;
                let tagged = tag
                    .tagger()
                    .ok()
                    .flatten()
                    .and_then(|tagger| tagger.time().ok());
                match tagged {
                    Some(time) => Some(time.seconds),
                    None => self.date_of(tag.target_id().ok()?.detach()),
                }
            }
            _ => None,
        }
    }
}
