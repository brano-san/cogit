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
    "rerere.enabled",
    "rerere.autoUpdate",
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
    /// Where the value in effect comes from, as `git config --show-scope --show-origin`
    /// names it: `global` and `file:C:/Users/me/.gitconfig`, or an `includeIf` file.
    pub scope: Option<String>,
    pub origin: Option<String>,
}

/// `-z --show-scope --show-origin`: scope, origin, then key and value split by a newline.
fn parse_origins(output: &str) -> std::collections::HashMap<String, (String, String)> {
    let fields: Vec<&str> = output.split('\0').collect();
    let mut found = std::collections::HashMap::new();
    for chunk in fields.as_chunks::<3>().0 {
        let key = chunk[2].split_once('\n').map_or(chunk[2], |(key, _)| key);
        found.insert(
            key.to_ascii_lowercase(),
            (chunk[0].to_owned(), chunk[1].to_owned()),
        );
    }
    found
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
        let origins = self.setting_origins();
        REPO_SETTING_KEYS
            .iter()
            .map(|key| {
                let origin = origins.get(&key.to_ascii_lowercase());
                RepoSetting {
                    scope: origin.map(|(scope, _)| scope.clone()),
                    origin: origin.map(|(_, origin)| origin.clone()),
                    key: (*key).to_owned(),
                    local: file
                        .string_filter(*key, |meta| is_local(meta.source))
                        .map(|value| value.to_string()),
                    inherited: file
                        .string_filter(*key, |meta| is_inherited(meta.source))
                        .map(|value| value.to_string()),
                }
            })
            .collect()
    }

    /// The last value git reads wins, so the last line for a key is the one in effect.
    fn setting_origins(&self) -> std::collections::HashMap<String, (String, String)> {
        let pattern = format!(
            "^({})$",
            REPO_SETTING_KEYS
                .iter()
                .map(|key| key.replace('.', "\\."))
                .collect::<Vec<_>>()
                .join("|")
        );
        let mut command = self.base_git(&[
            "config",
            "--show-scope",
            "--show-origin",
            "-z",
            "--get-regexp",
            &pattern,
        ]);
        match crate::children::output(&mut command) {
            Ok(output) if !matches!(output.status.code(), Some(0 | 1)) => {
                // Only stderr: stdout holds the config values (R-155).
                tracing::warn!(
                    exit_code = ?output.status.code(),
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    context = "config origins for repository settings"
                );
                std::collections::HashMap::new()
            }
            Ok(output) => parse_origins(&String::from_utf8_lossy(&output.stdout)),
            Err(err) => {
                tracing::warn!(error = ?err, context = "config origins for repository settings");
                std::collections::HashMap::new()
            }
        }
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

#[cfg(test)]
mod tests {
    use super::parse_origins;

    #[test]
    fn the_last_origin_of_a_key_is_the_one_in_effect() {
        let out = "global\0file:C:/u/.gitconfig\0user.name\nAnn\0local\0file:.git/config\0user.name\nBo\0";
        let found = parse_origins(out);
        assert_eq!(
            found.get("user.name"),
            Some(&("local".to_owned(), "file:.git/config".to_owned()))
        );
    }
}
