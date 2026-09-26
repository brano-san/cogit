//! A remote's own menu in Branches: Properties, Rename, Delete (#19 of 25.09).

use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

/// In the remote's own section, so `git remote rename` carries it and `git remote remove`
/// takes it away; absent means on (R-554).
const BACKGROUND_KEY: &str = "cogitBackgroundFetch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    /// `remote.<name>.url` as written, before `insteadOf`.
    pub url: Option<String>,
    pub push_url: Option<String>,
    /// Whether the Repositories check asks this remote's server (R-554).
    pub background_fetch: bool,
    /// The repository is a shallow clone, which Set Depth deepens.
    pub shallow: bool,
}

impl RepoHandle {
    pub fn remote_info(&self, name: &str) -> Result<RemoteInfo> {
        if !self.remotes()?.iter().any(|remote| remote == name) {
            return Err(GitError::InvalidState(format!(
                "There is no remote {name}."
            )));
        }
        let config = self.repo.config_snapshot();
        let value = |key: &str| {
            config
                .plumbing()
                .string_by("remote", Some(name.into()), key)
                .map(|value| value.to_string())
        };
        Ok(RemoteInfo {
            name: name.to_owned(),
            url: value("url"),
            push_url: value("pushurl"),
            background_fetch: self.background_fetch_of(name),
            shallow: self.repo.is_shallow(),
        })
    }

    /// A value git cannot read as a boolean counts as on: the check is the default.
    pub(crate) fn background_fetch_of(&self, name: &str) -> bool {
        match self.repo.config_snapshot().plumbing().boolean_by(
            "remote",
            Some(name.into()),
            BACKGROUND_KEY,
        ) {
            Ok(value) => value.unwrap_or(true),
            Err(err) => {
                tracing::error!(error = ?err, remote = name, context = "reading the background check of a remote");
                true
            }
        }
    }

    pub fn set_background_fetch(&self, name: &str, on: bool) -> Result<()> {
        let key = format!("remote.{name}.{BACKGROUND_KEY}");
        if !on {
            return self.run_git(&["config", &key, "false"]).map(drop);
        }
        if self.background_fetch_of(name) {
            return Ok(());
        }
        self.run_git(&["config", "--unset-all", &key]).map(drop)
    }

    /// `git remote rename`: its remote branches, the upstreams naming it and its own
    /// config section move with it.
    pub fn rename_remote(&self, from: &str, to: &str) -> Result<()> {
        self.run_git(&["remote", "rename", from, to]).map(drop)
    }

    /// `git remote remove`: its remote branches and upstreams go too; the server is not touched.
    pub fn remove_remote(&self, name: &str) -> Result<()> {
        self.run_git(&["remote", "remove", name]).map(drop)
    }

    /// Refused when it starts with `-`: `git remote set-url` would read it as an option.
    pub fn set_remote_url(&self, name: &str, url: &str) -> Result<()> {
        let url = url.trim();
        if url.is_empty() || url.starts_with('-') {
            return Err(GitError::InvalidState(format!(
                "{url:?} is not a URL or a path git can use."
            )));
        }
        self.run_git(&["remote", "set-url", name, url]).map(drop)
    }
}
