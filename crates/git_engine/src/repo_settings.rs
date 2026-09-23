use crate::{GitError, RepoHandle, Result};
use gix::config::Source;

/// What Repository ▸ Settings offers, in order; `cogit.*` keys are Cogit's own, git ignores them.
pub const REPO_SETTING_KEYS: &[&str] = &[
    "user.name",
    "user.email",
    "pull.rebase",
    "fetch.prune",
    "fetch.recurseSubmodules",
    "submodule.recurse",
    "cogit.initNewSubmodules",
    "push.default",
    "push.autoSetupRemote",
    "push.followTags",
    "commit.gpgSign",
    "tag.gpgSign",
    "user.signingKey",
    "gpg.format",
    "i18n.commitEncoding",
    "i18n.logOutputEncoding",
    "cogit.tagGroupSeparator",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoSetting {
    pub key: String,
    /// Set in this repository's config (`.git/config`, or `config.worktree`).
    pub local: Option<String>,
    /// What applies when `local` is unset: the user's and the system's config.
    pub inherited: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoSettingChange {
    pub key: String,
    /// `None` removes the key from the repository's config.
    pub value: Option<String>,
}

fn is_local(source: Source) -> bool {
    matches!(source, Source::Local | Source::Worktree)
}

fn is_inherited(source: Source) -> bool {
    matches!(
        source,
        Source::GitInstallation | Source::System | Source::Git | Source::User
    )
}

impl RepoHandle {
    #[must_use]
    pub fn repo_settings(&self) -> Vec<RepoSetting> {
        let snapshot = self.repo.config_snapshot();
        let file = snapshot.plumbing();
        REPO_SETTING_KEYS
            .iter()
            .map(|key| RepoSetting {
                key: (*key).to_owned(),
                local: file
                    .string_filter(*key, |meta| is_local(meta.source))
                    .map(|value| value.to_string()),
                inherited: file
                    .string_filter(*key, |meta| is_inherited(meta.source))
                    .map(|value| value.to_string()),
            })
            .collect()
    }

    pub fn write_repo_settings(&self, changes: &[RepoSettingChange]) -> Result<()> {
        if let Some(stranger) = changes.iter().find(|change| offered(&change.key).is_none()) {
            return Err(GitError::InvalidState(format!(
                "{} is not a repository setting",
                stranger.key
            )));
        }
        let current = self.repo_settings();
        for change in changes {
            let key = offered(&change.key).unwrap_or(&change.key);
            match &change.value {
                Some(value) => {
                    self.run_git(&["config", "--local", "--replace-all", "--", key, value])?;
                }
                // Asked first so that nothing already absent shows up as a failed command.
                None if current
                    .iter()
                    .any(|entry| entry.key == key && entry.local.is_some()) =>
                {
                    self.run_git(&["config", "--local", "--unset-all", "--", key])?;
                }
                None => {}
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn wants_new_submodules(&self) -> bool {
        self.repo
            .config_snapshot()
            .boolean("cogit.initNewSubmodules")
            .unwrap_or(false)
    }
}

fn offered(key: &str) -> Option<&'static str> {
    REPO_SETTING_KEYS
        .iter()
        .find(|known| known.eq_ignore_ascii_case(key))
        .copied()
}
