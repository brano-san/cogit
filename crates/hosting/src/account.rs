use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HostingProvider {
    Github,
    Gitlab,
    Bitbucket,
}

impl HostingProvider {
    #[must_use]
    pub fn cloud_url(self) -> &'static str {
        match self {
            Self::Github => "https://github.com",
            Self::Gitlab => "https://gitlab.com",
            Self::Bitbucket => "https://bitbucket.org",
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Github => "GitHub",
            Self::Gitlab => "GitLab",
            Self::Bitbucket => "Bitbucket",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostingAccount {
    pub id: String,
    pub provider: HostingProvider,
    pub server_url: Option<String>,
    pub login: String,
    #[serde(default)]
    pub username: Option<String>,
    pub use_for_git: bool,
}

impl HostingAccount {
    #[must_use]
    pub fn web_url(&self) -> String {
        self.server_url
            .clone()
            .unwrap_or_else(|| self.provider.cloud_url().to_owned())
    }

    #[must_use]
    pub fn api_url(&self) -> String {
        match (self.provider, &self.server_url) {
            (HostingProvider::Github, None) => "https://api.github.com".to_owned(),
            (HostingProvider::Github, Some(server)) => format!("{server}/api/v3"),
            (HostingProvider::Gitlab, _) => format!("{}/api/v4", self.web_url()),
            (HostingProvider::Bitbucket, _) => "https://api.bitbucket.org/2.0".to_owned(),
        }
    }

    #[must_use]
    pub fn host(&self) -> String {
        split_server(&self.web_url()).map_or_else(String::new, |(host, _)| host)
    }

    #[must_use]
    pub fn path_prefix(&self) -> String {
        split_server(&self.web_url()).map_or_else(String::new, |(_, prefix)| prefix)
    }

    #[must_use]
    pub fn git_username(&self) -> String {
        match self.provider {
            HostingProvider::Bitbucket => "x-bitbucket-api-token-auth".to_owned(),
            HostingProvider::Github | HostingProvider::Gitlab => self.login.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccountError {
    #[error("the server URL must start with https:// (http:// only for this machine)")]
    Scheme,
    #[error("the server URL has no host")]
    NoHost,
    #[error("Bitbucket accounts are for bitbucket.org only")]
    SelfHostedBitbucket,
    #[error("Bitbucket needs the email address of the Atlassian account")]
    NoUsername,
    #[error("an access token cannot be empty")]
    NoToken,
}

const LOOPBACK: &[&str] = &["127.0.0.1", "localhost", "[::1]"];

#[must_use]
pub fn is_loopback(host: &str) -> bool {
    LOOPBACK.contains(&host)
}

fn split_server(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        Some(inner) => format!("[{}]", inner.split_once(']')?.0),
        None => host
            .split_once(':')
            .map_or(host, |(name, _)| name)
            .to_owned(),
    };
    let prefix = path.trim_matches('/');
    Some((host.to_lowercase(), prefix.to_owned()))
}

pub fn normalize_server(
    provider: HostingProvider,
    text: Option<&str>,
) -> Result<Option<String>, AccountError> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let text = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let (scheme, rest) = text.split_once("://").ok_or(AccountError::Scheme)?;
    let scheme = scheme.to_lowercase();
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
        .to_lowercase();
    let (host, _) = split_server(&format!("https://{authority}")).ok_or(AccountError::NoHost)?;
    if host.is_empty() {
        return Err(AccountError::NoHost);
    }
    match scheme.as_str() {
        "https" => {}
        "http" if is_loopback(&host) => {}
        _ => return Err(AccountError::Scheme),
    }
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_matches('/');
    let url = if path.is_empty() {
        format!("{scheme}://{authority}")
    } else {
        format!("{scheme}://{authority}/{path}")
    };
    if url == provider.cloud_url() {
        return Ok(None);
    }
    if provider == HostingProvider::Bitbucket {
        return Err(AccountError::SelfHostedBitbucket);
    }
    Ok(Some(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_url_is_cut_to_scheme_host_and_path() {
        assert_eq!(
            normalize_server(HostingProvider::Gitlab, Some(" git.Corp.example/gitlab/ ")),
            Ok(Some("https://git.corp.example/gitlab".to_owned()))
        );
        assert_eq!(
            normalize_server(HostingProvider::Github, Some("https://github.com/")),
            Ok(None)
        );
        assert_eq!(
            normalize_server(HostingProvider::Github, Some("")),
            Ok(None)
        );
        assert_eq!(
            normalize_server(HostingProvider::Github, Some("http://ghe.example")),
            Err(AccountError::Scheme)
        );
        assert_eq!(
            normalize_server(HostingProvider::Github, Some("http://127.0.0.1:8080")),
            Ok(Some("http://127.0.0.1:8080".to_owned()))
        );
        assert_eq!(
            normalize_server(HostingProvider::Bitbucket, Some("https://bitbucket.corp")),
            Err(AccountError::SelfHostedBitbucket)
        );
    }

    #[test]
    fn api_urls_follow_the_provider() {
        let account = |provider, server: Option<&str>| HostingAccount {
            id: "a".into(),
            provider,
            server_url: server.map(str::to_owned),
            login: "ann".into(),
            username: None,
            use_for_git: true,
        };
        assert_eq!(
            account(HostingProvider::Github, None).api_url(),
            "https://api.github.com"
        );
        assert_eq!(
            account(HostingProvider::Github, Some("https://ghe.corp")).api_url(),
            "https://ghe.corp/api/v3"
        );
        assert_eq!(
            account(HostingProvider::Gitlab, None).api_url(),
            "https://gitlab.com/api/v4"
        );
        assert_eq!(
            account(HostingProvider::Gitlab, Some("https://corp/gitlab")).api_url(),
            "https://corp/gitlab/api/v4"
        );
        assert_eq!(
            account(HostingProvider::Bitbucket, None).api_url(),
            "https://api.bitbucket.org/2.0"
        );
        assert_eq!(
            account(HostingProvider::Gitlab, Some("https://corp:8443/gitlab")).host(),
            "corp"
        );
        assert_eq!(
            account(HostingProvider::Gitlab, Some("https://corp:8443/gitlab")).path_prefix(),
            "gitlab"
        );
    }
}
