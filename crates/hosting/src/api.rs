use crate::account::{AccountError, HostingAccount, HostingProvider, is_loopback};
use crate::remote::RemoteRef;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("{0} refused the token (HTTP {1}). Check that it is valid and not expired.")]
    Refused(String, u16),
    #[error("{0} answered HTTP {1}: {2}")]
    Status(String, u16, String),
    #[error("could not reach {0}: {1}")]
    Network(String, String),
    #[error("the proxy {0} is not a valid proxy URL")]
    Proxy(String),
    #[error("{0} limits how often Cogit may ask{wait}", wait = wait_text(.1))]
    RateLimited(String, Option<u64>),
    #[error("{0} sent an answer Cogit does not understand")]
    Reply(String),
    #[error(transparent)]
    Account(#[from] AccountError),
}

fn wait_text(wait: &Option<u64>) -> String {
    match *wait {
        Some(secs) if secs >= 120 => format!("; it allows more in {} min", secs.div_ceil(60)),
        Some(secs) => format!("; it allows more in {secs} s"),
        None => String::new(),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limit {
    pub remaining: Option<u64>,
    pub reset_at: Option<u64>,
    pub retry_after: Option<u64>,
}

impl Limit {
    #[must_use]
    pub fn wait_secs(&self, now_epoch: u64) -> Option<u64> {
        self.retry_after.or_else(|| {
            self.reset_at
                .filter(|_| self.remaining == Some(0))
                .map(|reset| reset.saturating_sub(now_epoch))
        })
    }

    fn exhausted(&self) -> bool {
        self.remaining == Some(0) || self.retry_after.is_some()
    }
}

#[derive(Debug)]
pub struct Reply {
    pub body: serde_json::Value,
    pub limit: Limit,
}

pub struct Auth(String);

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Auth(<redacted>)")
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn auth_for(
    provider: HostingProvider,
    username: Option<&str>,
    token: &str,
) -> Result<Auth, AccountError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AccountError::NoToken);
    }
    match provider {
        HostingProvider::Github | HostingProvider::Gitlab => Ok(Auth(format!("Bearer {token}"))),
        HostingProvider::Bitbucket => {
            let user = username
                .map(str::trim)
                .filter(|user| !user.is_empty())
                .ok_or(AccountError::NoUsername)?;
            Ok(Auth(format!(
                "Basic {}",
                base64(format!("{user}:{token}").as_bytes())
            )))
        }
    }
}

fn agent_for(url: &str, proxy: Option<&str>) -> Result<ureq::Agent, ApiError> {
    let local = RemoteRef::parse(url).is_some_and(|remote| is_loopback(&remote.host));
    let proxy = match proxy.map(str::trim).filter(|proxy| !proxy.is_empty()) {
        _ if local => None,
        Some(proxy) => Some(ureq::Proxy::new(proxy).map_err(|_| ApiError::Proxy(redact(proxy)))?),
        None => ureq::Proxy::try_from_env(),
    };
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .user_agent(concat!("cogit/", env!("CARGO_PKG_VERSION")))
        .proxy(proxy)
        .build();
    Ok(ureq::Agent::new_with_config(config))
}

fn redact(proxy: &str) -> String {
    match proxy.rsplit_once('@') {
        Some((userinfo, host)) => {
            let scheme = userinfo.split_once("://").map_or("", |(scheme, _)| scheme);
            let scheme = if scheme.is_empty() {
                String::new()
            } else {
                format!("{scheme}://")
            };
            format!("{scheme}<redacted>@{host}")
        }
        None => proxy.to_owned(),
    }
}

fn site(url: &str) -> String {
    RemoteRef::parse(url).map_or_else(|| url.to_owned(), |remote| remote.host)
}

pub fn get_json(
    url: &str,
    auth: &Auth,
    proxy: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    get_reply(url, auth, proxy, &[]).map(|reply| reply.body)
}

fn header_number(response: &ureq::http::Response<ureq::Body>, names: &[&str]) -> Option<u64> {
    names.iter().find_map(|name| {
        response
            .headers()
            .get(*name)?
            .to_str()
            .ok()?
            .trim()
            .parse()
            .ok()
    })
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub fn get_reply(
    url: &str,
    auth: &Auth,
    proxy: Option<&str>,
    headers: &[(&str, &str)],
) -> Result<Reply, ApiError> {
    let site = site(url);
    let started = std::time::Instant::now();
    let mut request = agent_for(url, proxy)?
        .get(url)
        .header("Authorization", &auth.0);
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("accept"))
    {
        request = request.header("Accept", "application/json");
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = match request.call() {
        Ok(response) => response,
        Err(error) => {
            tracing::info!(%site, ?error, "hosting API request did not complete");
            return Err(ApiError::Network(site, error.to_string()));
        }
    };
    let status = response.status().as_u16();
    let limit = Limit {
        remaining: header_number(&response, &["x-ratelimit-remaining", "ratelimit-remaining"]),
        reset_at: header_number(&response, &["x-ratelimit-reset", "ratelimit-reset"]),
        retry_after: header_number(&response, &["retry-after"]),
    };
    tracing::info!(%site, status, elapsed = ?started.elapsed(), "hosting API request");
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_string()
        .map_err(|error| ApiError::Network(site.clone(), error.to_string()))?;
    match status {
        200..=299 => serde_json::from_str(&body)
            .map(|body| Reply { body, limit })
            .map_err(|_| ApiError::Reply(site)),
        429 => Err(ApiError::RateLimited(site, limit.wait_secs(epoch_now()))),
        403 if limit.exhausted() => Err(ApiError::RateLimited(site, limit.wait_secs(epoch_now()))),
        401 | 403 => Err(ApiError::Refused(site, status)),
        _ => Err(ApiError::Status(
            site,
            status,
            body.chars().take(300).collect(),
        )),
    }
}

fn login_in(provider: HostingProvider, reply: &serde_json::Value) -> Option<String> {
    let keys: &[&str] = match provider {
        HostingProvider::Github => &["login"],
        HostingProvider::Gitlab => &["username"],
        HostingProvider::Bitbucket => &["username", "nickname", "display_name"],
    };
    keys.iter()
        .find_map(|key| reply.get(key)?.as_str().filter(|login| !login.is_empty()))
        .map(str::to_owned)
}

pub fn check_at(
    api_url: &str,
    provider: HostingProvider,
    username: Option<&str>,
    token: &str,
    proxy: Option<&str>,
) -> Result<String, ApiError> {
    let auth = auth_for(provider, username, token)?;
    let url = format!("{}/user", api_url.trim_end_matches('/'));
    let reply = get_json(&url, &auth, proxy)?;
    login_in(provider, &reply).ok_or_else(|| ApiError::Reply(site(&url)))
}

pub fn check(
    account: &HostingAccount,
    token: &str,
    proxy: Option<&str>,
) -> Result<String, ApiError> {
    check_at(
        &account.api_url(),
        account.provider,
        account.username.as_deref(),
        token,
        proxy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_like_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"ann@x.io:tok"), "YW5uQHguaW86dG9r");
    }

    #[test]
    fn a_proxy_password_is_not_shown() {
        assert_eq!(
            redact("http://ann:secret@proxy:3128"),
            "http://<redacted>@proxy:3128"
        );
        assert_eq!(redact("proxy:3128"), "proxy:3128");
    }

    #[test]
    fn a_limit_waits_for_retry_after_first_then_the_reset() {
        let spent = Limit {
            remaining: Some(0),
            reset_at: Some(1_000),
            retry_after: None,
        };
        assert_eq!(spent.wait_secs(940), Some(60));
        assert_eq!(spent.wait_secs(2_000), Some(0));
        let told = Limit {
            retry_after: Some(30),
            ..spent
        };
        assert_eq!(told.wait_secs(940), Some(30));
        let left = Limit {
            remaining: Some(5),
            ..spent
        };
        assert_eq!(left.wait_secs(940), None);
        assert!(!left.exhausted());
    }

    #[test]
    fn the_token_never_shows_in_debug() {
        let auth = auth_for(HostingProvider::Github, None, "ghp_secret").unwrap();
        assert!(!format!("{auth:?}").contains("ghp_secret"));
    }
}
