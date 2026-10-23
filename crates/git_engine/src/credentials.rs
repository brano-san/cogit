//! HTTP(S) sign-ins: how they reach git, and how git says they were not enough (F-082).

use serde::{Deserialize, Serialize};

/// No username: a token, sent under `x-access-token`. With `key`, an ssh private key and
/// `secret` its passphrase (empty: none).
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Credential {
    pub username: Option<String>,
    pub secret: String,
    pub key: Option<String>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("username", &self.username)
            .field("key", &self.key)
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
    // ssh: no key the server takes, or a wrong passphrase.
    "permission denied (publickey",
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

/// SSH carries its own credentials through the agent or a key, never an HTTP sign-in.
#[must_use]
pub fn wants_auth(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The first argument that makes Cogit's executable git's credential helper:
/// `<exe> cogit-credential <slot> get` (R-891).
pub const CREDENTIAL_HELPER_ARG: &str = "cogit-credential";
/// The passphrase of the key in `GIT_SSH_COMMAND`, for Cogit's executable as `SSH_ASKPASS`.
const SSH_PASSPHRASE_VAR: &str = "COGIT_SSH_PASSPHRASE";

fn slot_var(slot: &str, field: &str) -> String {
    format!("COGIT_CREDENTIAL_{slot}_{field}")
}

/// `scheme://host[:port]`, as git's credential config and protocol match it.
fn http_origin(url: &str) -> Option<String> {
    if !wants_auth(url) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    (!host.is_empty()).then(|| format!("{scheme}://{host}").to_lowercase())
}

/// Single-quoted for git's sh; on Windows with `/`, which its sh and ssh both read.
pub(crate) fn sh_quote(word: &str) -> String {
    let word = if cfg!(windows) {
        word.replace('\\', "/")
    } else {
        word.to_owned()
    };
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Only this key, not every key the agent holds; without a passphrase still never a question.
#[must_use]
pub fn ssh_command(key: &str, passphrase: bool) -> String {
    let mut command = format!(
        "ssh -i {} -o IdentitiesOnly=yes -o ConnectTimeout=20 -o ServerAliveInterval=15",
        sh_quote(key)
    );
    if !passphrase {
        command.push_str(" -o BatchMode=yes");
    }
    command
}

/// Each URL's credential for git, in the environment only: argv is visible to every user
/// (R-891). HTTP(S): Cogit as the only helper of that host; SSH: `GIT_SSH_COMMAND`.
#[must_use]
pub fn credential_env(signed: &[(String, Credential)]) -> Vec<(String, String)> {
    let Ok(exe) = std::env::current_exe() else {
        return Vec::new();
    };
    let exe = exe.to_string_lossy().into_owned();
    let mut config: Vec<(String, String)> = Vec::new();
    let mut env: Vec<(String, String)> = Vec::new();
    let mut origins: Vec<String> = Vec::new();
    for (url, credential) in signed {
        if let Some(key) = &credential.key {
            // One ssh per command: the first remote's key it is (ponytail: per-host keys via
            // a generated ssh config if two ssh remotes ever need two keys).
            if wants_auth(url) || env.iter().any(|(k, _)| k == "GIT_SSH_COMMAND") {
                continue;
            }
            let passphrase = !credential.secret.is_empty();
            env.push(("GIT_SSH_COMMAND".to_owned(), ssh_command(key, passphrase)));
            if passphrase {
                env.push(("SSH_ASKPASS".to_owned(), exe.clone()));
                env.push(("SSH_ASKPASS_REQUIRE".to_owned(), "force".to_owned()));
                env.push((SSH_PASSPHRASE_VAR.to_owned(), credential.secret.clone()));
            }
            continue;
        }
        let Some(origin) = http_origin(url).filter(|origin| !origins.contains(origin)) else {
            continue;
        };
        let slot = origins.len().to_string();
        let key = format!("credential.{origin}.helper");
        config.push((key.clone(), String::new()));
        config.push((
            key,
            format!("!{} {CREDENTIAL_HELPER_ARG} {slot}", sh_quote(&exe)),
        ));
        let username = credential.username.as_deref().unwrap_or("x-access-token");
        env.push((slot_var(&slot, "URL"), origin.clone()));
        env.push((slot_var(&slot, "USERNAME"), username.to_owned()));
        env.push((slot_var(&slot, "SECRET"), credential.secret.clone()));
        origins.push(origin);
    }
    let mut all = crate::network::config_env(&config);
    all.extend(env);
    all
}

/// Only `get`, only for the slot's `scheme://host`: a redirect elsewhere gets nothing.
fn credential_answer(
    action: &str,
    input: &str,
    slot: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if action != "get" {
        return None;
    }
    let field = |name: &str| {
        input
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
    };
    let asked = format!("{}://{}", field("protocol")?, field("host")?).to_lowercase();
    if asked != slot("URL")? {
        return None;
    }
    let (username, secret) = (slot("USERNAME")?, slot("SECRET")?);
    if [&username, &secret]
        .iter()
        .any(|value| value.contains(['\n', '\0']))
    {
        return None;
    }
    Some(format!("username={username}\npassword={secret}\n"))
}

/// Only the passphrase question is answered; host keys and passwords are refused (GR-06).
fn askpass_answer(prompt: &str, passphrase: Option<&str>) -> Option<String> {
    let passphrase = passphrase.filter(|p| !p.is_empty())?;
    prompt
        .to_lowercase()
        .contains("passphrase")
        .then(|| format!("{passphrase}\n"))
}

/// `Some(exit code)` when git or ssh started Cogit as their helper.
#[must_use]
pub fn run_as_helper() -> Option<i32> {
    use std::io::{Read as _, Write as _};
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(CREDENTIAL_HELPER_ARG) {
        let mut input = String::new();
        let _ = std::io::stdin().read_to_string(&mut input);
        let answer = args.get(1).zip(args.get(2)).and_then(|(slot, action)| {
            credential_answer(action, &input, |field| {
                std::env::var(slot_var(slot, field)).ok()
            })
        });
        if let Some(answer) = answer {
            let _ = std::io::stdout().write_all(answer.as_bytes());
        }
        return Some(0);
    }
    let passphrase = std::env::var(SSH_PASSPHRASE_VAR).ok();
    if passphrase.is_none() && std::env::var_os(crate::pulse::ASKPASS_REFUSAL_VAR).is_none() {
        return None;
    }
    let prompt = args.first().map_or("", String::as_str);
    match askpass_answer(prompt, passphrase.as_deref()) {
        Some(answer) => {
            let _ = std::io::stdout().write_all(answer.as_bytes());
            Some(0)
        }
        None => Some(1),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn token(secret: &str) -> Credential {
        Credential {
            username: None,
            secret: secret.to_owned(),
            key: None,
        }
    }

    fn key(path: &str, passphrase: &str) -> Credential {
        Credential {
            username: None,
            secret: passphrase.to_owned(),
            key: Some(path.to_owned()),
        }
    }

    fn get(env: &[(String, String)], key: &str) -> Option<String> {
        env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn a_secret_never_reaches_a_command_or_a_debug_line() {
        let login = Credential {
            username: Some("ann".to_owned()),
            ..token("s3cret")
        };
        assert!(!format!("{login:?}").contains("s3cret"));
        let env = credential_env(&[("https://u:p@H.example:8443/o/r.git".to_owned(), login)]);

        assert_eq!(get(&env, "GIT_CONFIG_COUNT").as_deref(), Some("2"));
        assert_eq!(
            get(&env, "GIT_CONFIG_KEY_0").as_deref(),
            Some("credential.https://h.example:8443.helper")
        );
        // The empty entry first: the user's helpers are not asked for this host.
        assert_eq!(get(&env, "GIT_CONFIG_VALUE_0").as_deref(), Some(""));
        let helper = get(&env, "GIT_CONFIG_VALUE_1").unwrap();
        assert!(helper.starts_with("!'"), "{helper}");
        assert!(helper.ends_with("' cogit-credential 0"), "{helper}");
        assert!(!helper.contains("s3cret"));
        assert_eq!(
            get(&env, "COGIT_CREDENTIAL_0_URL").as_deref(),
            Some("https://h.example:8443")
        );
        assert_eq!(
            get(&env, "COGIT_CREDENTIAL_0_USERNAME").as_deref(),
            Some("ann")
        );
        assert_eq!(
            get(&env, "COGIT_CREDENTIAL_0_SECRET").as_deref(),
            Some("s3cret")
        );
    }

    #[test]
    fn one_slot_per_host_and_none_for_ssh_or_a_path() {
        let env = credential_env(&[
            ("https://a.example/x.git".to_owned(), token("1")),
            ("https://a.example/y.git".to_owned(), token("2")),
            ("https://b.example/z.git".to_owned(), token("3")),
            ("git@github.com:o/r.git".to_owned(), token("4")),
            ("/srv/git/r.git".to_owned(), token("5")),
        ]);

        assert_eq!(get(&env, "GIT_CONFIG_COUNT").as_deref(), Some("4"));
        assert_eq!(get(&env, "COGIT_CREDENTIAL_0_SECRET").as_deref(), Some("1"));
        assert_eq!(get(&env, "COGIT_CREDENTIAL_1_SECRET").as_deref(), Some("3"));
        assert_eq!(
            get(&env, "COGIT_CREDENTIAL_0_USERNAME").as_deref(),
            Some("x-access-token")
        );
        assert!(get(&env, "GIT_SSH_COMMAND").is_none());
    }

    #[test]
    fn an_ssh_key_goes_to_ssh_and_its_passphrase_to_the_askpass() {
        let plain = credential_env(&[("git@h.example:o/r.git".to_owned(), key("/k/id", ""))]);
        assert_eq!(
            get(&plain, "GIT_SSH_COMMAND").as_deref(),
            Some(
                "ssh -i '/k/id' -o IdentitiesOnly=yes -o ConnectTimeout=20 -o ServerAliveInterval=15 -o BatchMode=yes"
            )
        );
        assert!(get(&plain, "SSH_ASKPASS").is_none());

        let locked = credential_env(&[(
            "ssh://git@h.example/o/r.git".to_owned(),
            key("/k/id", "pass phrase"),
        )]);
        let command = get(&locked, "GIT_SSH_COMMAND").unwrap();
        assert!(!command.contains("BatchMode"), "{command}");
        assert!(!command.contains("pass phrase"), "{command}");
        assert_eq!(
            get(&locked, "SSH_ASKPASS_REQUIRE").as_deref(),
            Some("force")
        );
        assert_eq!(
            get(&locked, "COGIT_SSH_PASSPHRASE").as_deref(),
            Some("pass phrase")
        );
        // A key is no HTTP sign-in.
        assert!(credential_env(&[("https://h.example/r".to_owned(), key("/k", ""))]).is_empty());
    }

    // Git runs core.sshCommand through its sh: what ssh receives is checked through the
    // same sh, with an alias standing in for ssh.
    #[test]
    fn a_key_path_with_spaces_quotes_and_backslashes_arrives_whole() {
        let path = if cfg!(windows) {
            r"C:\Users\Ann O'Neil\.ssh\id_ed25519"
        } else {
            "/home/Ann O'Neil/.ssh/id $x"
        };
        let alias = format!("alias.probe=!printf '[%s]\\n' -i {}", sh_quote(path));
        let out = crate::runner::bare_git(&["-c", &alias, "probe"]).unwrap();
        let expected = if cfg!(windows) {
            "[-i]\n[C:/Users/Ann O'Neil/.ssh/id_ed25519]\n"
        } else {
            "[-i]\n[/home/Ann O'Neil/.ssh/id $x]\n"
        };
        assert_eq!(out.stdout, expected, "{}", out.stderr);
    }

    #[test]
    fn the_helper_answers_get_for_its_host_only() {
        let slot = |field: &str| {
            Some(
                match field {
                    "URL" => "https://h.example:8443",
                    "USERNAME" => "ann",
                    _ => "s3cret",
                }
                .to_owned(),
            )
        };
        let asked = "protocol=https\nhost=H.example:8443\npath=o/r.git\n\n";
        assert_eq!(
            credential_answer("get", asked, slot).as_deref(),
            Some("username=ann\npassword=s3cret\n")
        );
        assert_eq!(credential_answer("store", asked, slot), None);
        assert_eq!(credential_answer("erase", asked, slot), None);
        // A redirect elsewhere, another scheme, another port: nothing.
        for other in [
            "protocol=https\nhost=evil.example\n",
            "protocol=http\nhost=h.example:8443\n",
            "protocol=https\nhost=h.example\n",
            "host=h.example:8443\n",
        ] {
            assert_eq!(credential_answer("get", other, slot), None, "{other}");
        }
        let broken = |field: &str| {
            (field == "SECRET")
                .then(|| "a\nb".to_owned())
                .or(slot(field))
        };
        assert_eq!(credential_answer("get", asked, broken), None);
    }

    #[test]
    fn the_askpass_answers_a_passphrase_and_nothing_else() {
        let passphrase = Some("p");
        assert_eq!(
            askpass_answer("Enter passphrase for key '/k/id': ", passphrase).as_deref(),
            Some("p\n")
        );
        for prompt in [
            "Are you sure you want to continue connecting (yes/no/[fingerprint])?",
            "git@h.example's password: ",
        ] {
            assert_eq!(askpass_answer(prompt, passphrase), None, "{prompt}");
        }
        assert_eq!(askpass_answer("Enter passphrase for key", None), None);
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
                "git@h.example: Permission denied (publickey).\nfatal: Could not read from remote repository.",
                Some(Refused),
            ),
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
