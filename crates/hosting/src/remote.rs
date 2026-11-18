use crate::account::HostingAccount;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRef {
    pub host: String,
    pub path: String,
    pub user: Option<String>,
    pub https: bool,
}

const SCHEMES: &[(&str, bool)] = &[
    ("https://", true),
    ("http://", true),
    ("ssh://", false),
    ("git+ssh://", false),
    ("ssh+git://", false),
    ("git://", false),
];

impl RemoteRef {
    #[must_use]
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        let lower = url.to_lowercase();
        let scheme = SCHEMES.iter().find(|(scheme, _)| lower.starts_with(scheme));
        let (authority, path, https) = match scheme {
            Some((scheme, https)) => {
                let rest = &url[scheme.len()..];
                let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
                (authority, path, *https)
            }
            None if url.contains("://") => return None,
            None => {
                let (authority, path) = url.split_once(':')?;
                if authority.len() < 2 || authority.contains(['/', '\\']) {
                    return None;
                }
                (authority, path, false)
            }
        };
        let (user, host) = match authority.rsplit_once('@') {
            Some((userinfo, host)) => {
                let user = userinfo.split_once(':').map_or(userinfo, |(user, _)| user);
                (Some(user.to_owned()).filter(|user| !user.is_empty()), host)
            }
            None => (None, authority),
        };
        let host = match host.strip_prefix('[') {
            Some(inner) => format!("[{}]", inner.split_once(']')?.0),
            None => host
                .split_once(':')
                .map_or(host, |(name, _)| name)
                .to_owned(),
        };
        if host.is_empty() {
            return None;
        }
        let path = path
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_matches('/');
        let path = path
            .strip_suffix(".git")
            .unwrap_or(path)
            .trim_end_matches('/');
        Some(Self {
            host: host.to_lowercase(),
            path: path.to_owned(),
            user,
            https,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostingMatch {
    pub account: HostingAccount,
    pub repository: String,
    pub web_url: String,
    pub api_url: String,
}

fn repository_of(account: &HostingAccount, remote: &RemoteRef) -> Option<String> {
    if account.host() != remote.host {
        return None;
    }
    let prefix = account.path_prefix();
    let repository = if remote.https && !prefix.is_empty() {
        remote.path.strip_prefix(&prefix)?.strip_prefix('/')?
    } else {
        remote.path.as_str()
    };
    (!repository.is_empty()).then(|| repository.to_owned())
}

fn rank(account: &HostingAccount, remote: &RemoteRef, repository: &str) -> u8 {
    let login = account.login.as_str();
    let user = remote.user.as_deref().filter(|user| *user != "git");
    if user.is_some_and(|user| user.eq_ignore_ascii_case(login)) {
        return 0;
    }
    let owner = repository.split('/').next().unwrap_or("");
    if owner.eq_ignore_ascii_case(login) {
        return 1;
    }
    2
}

#[must_use]
pub fn account_for(accounts: &[HostingAccount], url: &str) -> Option<HostingMatch> {
    let remote = RemoteRef::parse(url)?;
    accounts
        .iter()
        .filter_map(|account| {
            let repository = repository_of(account, &remote)?;
            Some((rank(account, &remote, &repository), account, repository))
        })
        .min_by_key(|(rank, _, _)| *rank)
        .map(|(_, account, repository)| HostingMatch {
            web_url: format!("{}/{repository}", account.web_url()),
            api_url: account.api_url(),
            account: account.clone(),
            repository,
        })
}
