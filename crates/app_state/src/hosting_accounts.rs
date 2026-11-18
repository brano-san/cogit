use crate::AppState;
use crate::credentials::{SecretError, Stored, change_entries_in, parse, with_store_in};
use crate::password_store::{StoreError, StoreState};
use git_engine::Credential;
use hosting::{AccountError, ApiError, HostingAccount, HostingMatch, HostingProvider};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum HostingError {
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Account(#[from] AccountError),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("no such hosting account")]
    NotFound,
}

impl From<StoreError> for HostingError {
    fn from(err: StoreError) -> Self {
        Self::Secret(err.into())
    }
}

#[derive(Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewHostingAccount {
    pub provider: HostingProvider,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub token: String,
    pub use_for_git: bool,
}

impl std::fmt::Debug for NewHostingAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewHostingAccount")
            .field("provider", &self.provider)
            .field("server_url", &self.server_url)
            .field("username", &self.username)
            .field("use_for_git", &self.use_for_git)
            .finish_non_exhaustive()
    }
}

impl Drop for NewHostingAccount {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.token);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostingAccounts {
    pub state: StoreState,
    pub accounts: Vec<HostingAccount>,
}

fn accounts_in(entries: &[Stored]) -> Vec<HostingAccount> {
    entries
        .iter()
        .filter_map(|entry| entry.account.clone())
        .collect()
}

pub(crate) fn git_credential(entries: &[Stored], url: &str) -> Option<Credential> {
    let usable: Vec<HostingAccount> = accounts_in(entries)
        .into_iter()
        .filter(|account| account.use_for_git)
        .collect();
    let found = hosting::account_for(&usable, url)?;
    let entry = entries.iter().find(|entry| {
        entry
            .account
            .as_ref()
            .is_some_and(|a| a.id == found.account.id)
    })?;
    Some(Credential {
        username: Some(found.account.git_username()),
        secret: entry.secret.clone(),
        key: None,
    })
}

fn new_id() -> String {
    let mut bytes = [0u8; 8];
    if let Err(err) = getrandom::fill(&mut bytes) {
        tracing::error!(error = ?err, context = "a random hosting account id; the clock instead");
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        return format!("{nanos:x}");
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn proxy() -> Option<String> {
    match git_engine::global_network_settings() {
        Ok(settings) => settings
            .into_iter()
            .find(|setting| setting.key == "http.proxy")
            .and_then(|setting| setting.value),
        Err(err) => {
            tracing::error!(error = ?err, context = "reading http.proxy for a hosting API call");
            None
        }
    }
}

fn same_account(a: &HostingAccount, b: &HostingAccount) -> bool {
    a.provider == b.provider
        && a.server_url == b.server_url
        && a.login.eq_ignore_ascii_case(&b.login)
}

impl AppState {
    fn read_entries(&self) -> Result<Vec<Stored>, HostingError> {
        Ok(with_store_in(&self.passwords, |store| {
            parse(&store.read()?)
        })?)
    }

    pub fn hosting_accounts(&self) -> Result<HostingAccounts, HostingError> {
        let state = with_store_in(&self.passwords, |store| Ok(store.state()))?;
        let accounts = if state == StoreState::Unlocked {
            accounts_in(&self.read_entries()?)
        } else {
            Vec::new()
        };
        Ok(HostingAccounts { state, accounts })
    }

    pub fn add_hosting_account(
        &self,
        new: &NewHostingAccount,
    ) -> Result<HostingAccount, HostingError> {
        let server_url = hosting::normalize_server(new.provider, new.server_url.as_deref())?;
        let username = new
            .username
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty() && new.provider == HostingProvider::Bitbucket)
            .map(str::to_owned);
        let token = Zeroizing::new(new.token.trim().to_owned());
        let mut account = HostingAccount {
            id: new_id(),
            provider: new.provider,
            server_url,
            login: String::new(),
            username,
            use_for_git: new.use_for_git,
        };
        with_store_in(&self.passwords, |store| match store.state() {
            StoreState::Unlocked => Ok(()),
            StoreState::Locked => Err(StoreError::Locked.into()),
            StoreState::Missing => Err(SecretError::NoStore),
        })?;
        account.login = hosting::check(&account, &token, proxy().as_deref())?;
        let stored = account.clone();
        change_entries_in(&self.passwords, |entries| {
            let replaced = entries.iter_mut().find(|entry| {
                entry
                    .account
                    .as_ref()
                    .is_some_and(|old| same_account(old, &stored))
            });
            match replaced {
                Some(entry) => {
                    let id = entry.account.as_ref().map(|old| old.id.clone());
                    entry.secret = token.to_string();
                    entry.account = Some(HostingAccount {
                        id: id.unwrap_or_else(|| stored.id.clone()),
                        ..stored.clone()
                    });
                }
                None => entries.push(Stored {
                    host: stored.host(),
                    username: Some(stored.login.clone()),
                    secret: token.to_string(),
                    key: None,
                    account: Some(stored.clone()),
                }),
            }
        })?;
        let saved = self
            .hosting_accounts()?
            .accounts
            .into_iter()
            .find(|saved| same_account(saved, &account))
            .ok_or(HostingError::NotFound)?;
        tracing::info!(provider = ?saved.provider, host = %saved.host(), "hosting account added");
        Ok(saved)
    }

    pub fn check_hosting_account(&self, id: &str) -> Result<String, HostingError> {
        let entries = self.read_entries()?;
        let (account, token) = entries
            .iter()
            .find_map(|entry| {
                let account = entry.account.as_ref().filter(|account| account.id == id)?;
                Some((account.clone(), Zeroizing::new(entry.secret.clone())))
            })
            .ok_or(HostingError::NotFound)?;
        let login = hosting::check(&account, &token, proxy().as_deref())?;
        if login != account.login {
            change_entries_in(&self.passwords, |entries| {
                for entry in entries.iter_mut() {
                    if let Some(stored) = entry.account.as_mut().filter(|stored| stored.id == id) {
                        stored.login.clone_from(&login);
                        entry.username = Some(login.clone());
                    }
                }
            })?;
        }
        Ok(login)
    }

    pub fn remove_hosting_account(&self, id: &str) -> Result<(), HostingError> {
        Ok(change_entries_in(&self.passwords, |entries| {
            entries.retain(|entry| {
                entry
                    .account
                    .as_ref()
                    .is_none_or(|account| account.id != id)
            });
        })?)
    }

    pub fn hosting_account_for(&self, url: &str) -> Result<Option<HostingMatch>, HostingError> {
        Ok(hosting::account_for(
            &accounts_in(&self.read_entries()?),
            url,
        ))
    }

    pub fn hosting_token(&self, id: &str) -> Result<Zeroizing<String>, HostingError> {
        self.read_entries()?
            .into_iter()
            .find(|entry| {
                entry
                    .account
                    .as_ref()
                    .is_some_and(|account| account.id == id)
            })
            .map(|entry| Zeroizing::new(entry.secret))
            .ok_or(HostingError::NotFound)
    }
}

#[must_use]
pub fn hosting_proxy() -> Option<String> {
    proxy()
}
