//! HTTP(S) sign-ins: how they reach git, and how git says they were not enough (F-082).

use serde::{Deserialize, Serialize};

/// No username: a token, sent under `x-access-token`.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Credential {
    pub username: Option<String>,
    pub secret: String,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// How a network command's credential step ended, read off git's own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CredentialTrouble {
    /// Git Credential Manager's window was closed.
    Canceled,
    NoPrompt,
    Refused,
}

/// GCM's words come first: git's "could not read Username" follows them.
const CANCELED: &[&str] = &[
    "user canceled dialog",
    "user canceled the dialog",
    "canceled by the user",
    "authentication canceled",
    "authentication was canceled",
    "sign-in canceled",
    "sign-in was canceled",
];
const NO_PROMPT: &[&str] = &[
    "terminal prompts disabled",
    "could not read username",
    "could not read password",
    "cannot prompt because",
];
/// Status codes only as git and curl spell them: a bare `403` is also `(403/403)` progress.
const REFUSED: &[&str] = &[
    "authentication failed",
    "invalid username or password",
    "access denied",
    "returned error: 401",
    "returned error: 403",
    "http 401",
    "http 403",
    "401 unauthorized",
    "403 forbidden",
];

#[must_use]
pub fn credential_trouble(output: &str) -> Option<CredentialTrouble> {
    let text = output
        .to_lowercase()
        .replace("cancelled", "canceled")
        .replace("signin", "sign-in")
        .replace("sign in", "sign-in");
    let any = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    if any(CANCELED) {
        Some(CredentialTrouble::Canceled)
    } else if any(NO_PROMPT) {
        Some(CredentialTrouble::NoPrompt)
    } else if any(REFUSED) && !text.contains(" denied to ") {
        // GitHub's "Permission to o/r denied to ann": signed in, just not allowed.
        Some(CredentialTrouble::Refused)
    } else {
        None
    }
}

/// The URL git names in its refusal, for `fetch --all` and the like that ask several.
#[must_use]
pub fn refused_url(output: &str) -> Option<String> {
    let start = output.find("'http")? + 1;
    let end = output[start..].find('\'')?;
    Some(output[start..start + end].to_owned())
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (position, index) in [n >> 18, (n >> 12) & 63, (n >> 6) & 63, n & 63]
            .iter()
            .enumerate()
        {
            if position > chunk.len() {
                out.push('=');
            } else {
                out.push(char::from(ALPHABET[*index as usize]));
            }
        }
    }
    out
}

/// `x-access-token` is the username GitHub documents for a personal access token, and
/// GitLab and Bitbucket ignore the username when the password is a token.
#[must_use]
pub fn basic_header(credential: &Credential) -> String {
    let username = credential.username.as_deref().unwrap_or("x-access-token");
    format!(
        "Authorization: Basic {}",
        base64(format!("{username}:{}", credential.secret).as_bytes())
    )
}

/// SSH carries its own credentials through the agent, so a header there is noise at best.
#[must_use]
pub fn wants_auth(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The config entry (key, value) that hands `credential` to git for the host of `url`
/// only. Git passes its config on to the git processes it starts, a submodule's fetch
/// among them, so a bare `http.extraHeader` went to every host that fetch contacted.
#[must_use]
pub fn credential_config(url: &str, credential: &Credential) -> Option<(String, String)> {
    if !wants_auth(url) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host.is_empty() {
        return None;
    }
    Some((
        format!("http.{scheme}://{host}/.extraHeader"),
        basic_header(credential),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn token(secret: &str) -> Credential {
        Credential {
            username: None,
            secret: secret.to_owned(),
        }
    }

    #[test]
    fn a_credential_without_a_username_is_a_token() {
        assert_eq!(
            basic_header(&token("t")),
            format!("Authorization: Basic {}", base64(b"x-access-token:t"))
        );
    }

    #[test]
    fn a_credential_with_a_username_is_a_basic_login() {
        let login = Credential {
            username: Some("ann".to_owned()),
            secret: "s3cret".to_owned(),
        };
        assert_eq!(
            basic_header(&login),
            "Authorization: Basic YW5uOnMzY3JldA=="
        );
        assert!(!format!("{login:?}").contains("s3cret"));
    }

    #[test]
    fn a_credential_is_scoped_to_the_host_of_its_url() {
        let (key, value) =
            credential_config("https://u:p@h.example:8443/o/r.git", &token("t")).unwrap();

        assert_eq!(key, "http.https://h.example:8443/.extraHeader");
        assert_eq!(value, basic_header(&token("t")));
        assert!(credential_config("git@github.com:o/r.git", &token("t")).is_none());
    }

    #[test]
    fn git_words_tell_how_the_credential_step_ended() {
        use CredentialTrouble::{Canceled, NoPrompt, Refused};
        let cases = [
            (
                "fatal: User cancelled dialog.\nfatal: could not read Username for 'https://x': terminal prompts disabled",
                Some(Canceled),
            ),
            (
                "fatal: could not read Username for 'https://x': terminal prompts disabled",
                Some(NoPrompt),
            ),
            (
                "fatal: Authentication failed for 'https://x/o/r.git/'",
                Some(Refused),
            ),
            (
                "fatal: unable to access 'https://x/': The requested URL returned error: 401",
                Some(Refused),
            ),
            (
                "error: RPC failed; HTTP 403 curl 22 The requested URL returned error: 403",
                Some(Refused),
            ),
            ("remote: HTTP Basic: Access denied", Some(Refused)),
            (
                "Receiving objects: 100% (403/403), done.\nfatal: refusing to merge unrelated histories",
                None,
            ),
            ("fatal: repository 'https://x/o/r.git/' not found", None),
            // Signed in, but not allowed: another sign-in is not the answer.
            (
                "remote: Permission to o/r.git denied to ann.\nfatal: unable to access 'https://github.com/o/r.git/': The requested URL returned error: 403",
                None,
            ),
            (
                "fatal: unable to access 'https://x/': Could not resolve host: x",
                None,
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(credential_trouble(text), expected, "{text}");
        }
    }

    // `fetch --all` asks every remote: the one that refused is the one git names.
    #[test]
    fn the_refused_url_is_the_one_git_quotes() {
        assert_eq!(
            refused_url("Fetching upstream\nfatal: could not read Username for 'https://b.example': terminal prompts disabled").as_deref(),
            Some("https://b.example")
        );
        assert_eq!(
            refused_url("fatal: Authentication failed for 'https://a.example/o/r.git/'").as_deref(),
            Some("https://a.example/o/r.git/")
        );
        assert_eq!(refused_url("fatal: User cancelled dialog."), None);
    }
}
