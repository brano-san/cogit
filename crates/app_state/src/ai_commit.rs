use crate::credentials::{SecretError, Stored, change_entries_in, parse, with_store_in};
use crate::password_store::{StoreError, StoreState};
use crate::{AppState, RepoId};
use ai_message::{AiConfig, AiError, AiProvider, Change, Skipped, Staged};
use git_engine::{GitError, MessageContext, Omitted};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const SUBJECTS: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum AiCommitError {
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error(transparent)]
    Git(#[from] GitError),
}

impl From<StoreError> for AiCommitError {
    fn from(err: StoreError) -> Self {
        Self::Secret(err.into())
    }
}

impl From<AiCommitError> for GitError {
    fn from(err: AiCommitError) -> Self {
        match err {
            AiCommitError::Git(err) => err,
            AiCommitError::Ai(AiError::Cancelled) => {
                Self::Cancelled("AI commit message".to_owned())
            }
            other => Self::InvalidState(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiKeyState {
    pub state: StoreState,
    pub host: String,
    pub service: String,
    pub required: bool,
    pub stored: bool,
}

#[derive(Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiKeyInput {
    pub config: AiConfig,
    pub key: String,
}

impl std::fmt::Debug for AiKeyInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiKeyInput")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Drop for AiKeyInput {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.key);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiMessageRequest {
    pub config: AiConfig,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub message: String,
    pub shortened: bool,
}

fn skipped(omitted: Omitted) -> Skipped {
    match omitted {
        Omitted::Binary => Skipped::Binary,
        Omitted::Lockfile => Skipped::Lockfile,
        Omitted::OverBudget => Skipped::OverBudget,
    }
}

#[must_use]
pub fn staged_of(context: MessageContext) -> Staged {
    Staged {
        branch: context.branch,
        subjects: context.subjects,
        changes: context
            .files
            .into_iter()
            .map(|file| Change {
                path: file.path,
                old_path: file.old_path,
                added: file.added,
                deleted: file.deleted,
                skipped: file.omitted.map(skipped),
            })
            .collect(),
        diff: context.diff,
    }
}

fn is_key_for(entry: &Stored, provider: AiProvider, host: &str) -> bool {
    entry.ai == Some(provider) && entry.host == host
}

impl AppState {
    pub(crate) fn local_message_context(
        &self,
        repo: RepoId,
        subjects: usize,
    ) -> Result<MessageContext, GitError> {
        self.handle(repo)?.message_context(subjects)
    }

    pub fn ai_key_state(&self, config: &AiConfig) -> Result<AiKeyState, AiCommitError> {
        let host = config.host()?;
        let service = config.service_id()?;
        let (state, stored) = match with_store_in(&self.passwords, |store| {
            let state = store.state();
            let stored = state == StoreState::Unlocked
                && parse(&store.read()?)?
                    .iter()
                    .any(|entry| is_key_for(entry, config.provider, &service));
            Ok((state, stored))
        }) {
            Ok(state) => state,
            Err(SecretError::NoStore) => (StoreState::Missing, false),
            Err(err) => return Err(err.into()),
        };
        Ok(AiKeyState {
            state,
            host,
            service,
            required: config.needs_key()?,
            stored,
        })
    }

    pub fn set_ai_key(&self, input: &AiKeyInput) -> Result<(), AiCommitError> {
        let service = input.config.service_id()?;
        let key = Zeroizing::new(input.key.trim().to_owned());
        if key.is_empty() {
            return Err(SecretError::Empty.into());
        }
        let provider = input.config.provider;
        change_entries_in(&self.passwords, |entries| {
            entries.retain(|entry| !is_key_for(entry, provider, &service));
            entries.push(Stored {
                host: service,
                username: None,
                secret: key.to_string(),
                key: None,
                account: None,
                ai: Some(provider),
            });
        })?;
        tracing::info!(provider = ?provider, "AI provider key saved");
        Ok(())
    }

    pub fn remove_ai_key(&self, config: &AiConfig) -> Result<(), AiCommitError> {
        let service = config.service_id()?;
        Ok(change_entries_in(&self.passwords, |entries| {
            entries.retain(|entry| !is_key_for(entry, config.provider, &service));
        })?)
    }

    fn ai_key(&self, config: &AiConfig) -> Result<Option<Zeroizing<String>>, AiCommitError> {
        let service = config.service_id()?;
        let entries = match with_store_in(&self.passwords, |store| parse(&store.read()?)) {
            Ok(entries) => entries,
            Err(SecretError::NoStore) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        Ok(entries
            .into_iter()
            .find(|entry| is_key_for(entry, config.provider, &service))
            .map(|entry| Zeroizing::new(entry.secret)))
    }

    pub fn generate_commit_message(
        &self,
        repo: RepoId,
        request: &AiMessageRequest,
        stopped: &dyn Fn() -> bool,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<AiMessage, AiCommitError> {
        if stopped() {
            return Err(AiError::Cancelled.into());
        }
        let key = self.ai_key(&request.config)?;
        let context = self.message_context(repo, SUBJECTS)?;
        if context.files.is_empty() {
            return Err(GitError::InvalidState("nothing is staged".to_owned()).into());
        }
        let prompt = ai_message::build_with_budget(
            &staged_of(context),
            request.prefix.as_deref(),
            &request.config.instructions,
            request.config.max_diff_bytes.clamp(1_024, 256_000) as usize,
        );
        let outcome = ai_message::generate(
            &request.config,
            key.as_deref().map(String::as_str),
            &prompt,
            crate::hosting_proxy().as_deref(),
            stopped,
            on_text,
        )?;
        Ok(AiMessage {
            message: ai_message::clean_message(&outcome.text),
            shortened: prompt.shortened,
        })
    }

    pub fn test_ai_provider(&self, config: &AiConfig) -> Result<(), AiCommitError> {
        let key = self.ai_key(config)?;
        Ok(ai_message::test_provider(
            config,
            key.as_deref().map(String::as_str),
            crate::hosting_proxy().as_deref(),
        )?)
    }
}
