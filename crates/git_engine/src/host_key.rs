use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError, RwLock};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyQuestion {
    pub host: String,
    pub address: Option<String>,
    pub key_type: String,
    pub fingerprint: String,
    pub other_types_known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshPrompt {
    UnknownHost(HostKeyQuestion),
    ChangedHost,
    Other,
}

pub type HostKeyAsker = Arc<dyn Fn(&HostKeyQuestion, &dyn Fn() -> bool) -> bool + Send + Sync>;

pub const HOST_KEY_ASK_LIMIT: Duration = Duration::from_secs(120);

const ENDPOINT_VAR: &str = "COGIT_HOST_KEY_ENDPOINT";
const TOKEN_VAR: &str = "COGIT_HOST_KEY_TOKEN";
pub(crate) const REPOSITORY_ASKPASS: &str = "COGIT_REPOSITORY_ASKPASS";
const MAX_REQUEST: u64 = 16 * 1024;

pub(crate) const ASKING_SSH: &str =
    "core.sshCommand=ssh -o ConnectTimeout=20 -o ServerAliveInterval=15";

const CHANGED: &[&str] = &[
    "remote host identification has changed",
    "possible dns spoofing",
    "differs from the key for the ip address",
    "offending",
    "has changed",
    "revoked",
];

static ASKER: RwLock<Option<HostKeyAsker>> = RwLock::new(None);
static DESK: OnceLock<Option<Desk>> = OnceLock::new();

#[derive(Serialize, Deserialize)]
struct Desk {
    endpoint: String,
    token: String,
}

#[derive(Serialize, Deserialize)]
struct Asked {
    token: String,
    question: HostKeyQuestion,
}

#[derive(Serialize, Deserialize)]
struct RepositoryPrompt {
    token: String,
    prompt: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Incoming {
    Host(Asked),
    Repository(RepositoryPrompt),
}

#[derive(Clone)]
struct Login {
    interactive: bool,
    passphrase: Option<String>,
}

static LOGINS: LazyLock<Mutex<HashMap<String, Login>>> = LazyLock::new(Mutex::default);

pub struct RepositoryAskpass {
    file: tempfile::NamedTempFile,
    token: String,
}

impl std::fmt::Debug for RepositoryAskpass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryAskpass").finish_non_exhaustive()
    }
}

impl Drop for RepositoryAskpass {
    fn drop(&mut self) {
        LOGINS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.token);
    }
}

#[must_use]
pub fn read_prompt(prompt: &str) -> SshPrompt {
    let text = prompt.replace("\r\n", "\n").replace('\r', "\n");
    let lower = text.to_lowercase();
    if CHANGED.iter().any(|marker| lower.contains(marker)) {
        return SshPrompt::ChangedHost;
    }
    unknown_host(&text).map_or(SshPrompt::Other, SshPrompt::UnknownHost)
}

fn unknown_host(text: &str) -> Option<HostKeyQuestion> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let last = lines.last()?;
    if !last.starts_with("Are you sure you want to continue connecting (yes/no") {
        return None;
    }
    let named = lines.iter().find_map(|line| {
        let rest = line.strip_prefix("The authenticity of host '")?;
        let (named, tail) = rest.rsplit_once("' can't be established")?;
        tail.trim_end_matches('.').is_empty().then_some(named)
    })?;
    let (host, address) = split_host(named)?;
    let mut keys = lines.iter().filter_map(|line| {
        let (key_type, fingerprint) = line.split_once(" key fingerprint is ")?;
        Some((
            key_type,
            fingerprint.strip_suffix('.').unwrap_or(fingerprint),
        ))
    });
    let (key_type, fingerprint) = keys.next()?;
    if keys.next().is_some() || !valid_key_type(key_type) || !valid_fingerprint(fingerprint) {
        return None;
    }
    Some(HostKeyQuestion {
        host,
        address,
        key_type: key_type.to_owned(),
        fingerprint: fingerprint.to_owned(),
        other_types_known: lines
            .iter()
            .any(|line| line.contains("keys of different type are already known")),
    })
}

fn split_host(named: &str) -> Option<(String, Option<String>)> {
    let (host, address) = match named.strip_suffix(')').and_then(|n| n.rsplit_once(" (")) {
        Some((host, address)) => (host, Some(address)),
        None => (named, None),
    };
    let clean = |text: &str| !text.is_empty() && !text.chars().any(|c| c.is_control() || c == '\'');
    if !clean(host) || host.chars().count() > 300 {
        return None;
    }
    let address = address
        .filter(|address| !address.starts_with('<') && clean(address))
        .map(str::to_owned);
    Some((host.to_owned(), address))
}

fn valid_key_type(key_type: &str) -> bool {
    !key_type.is_empty()
        && key_type.len() <= 32
        && key_type
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn valid_fingerprint(fingerprint: &str) -> bool {
    if let Some(hash) = fingerprint.strip_prefix("SHA256:") {
        return (40..=48).contains(&hash.len())
            && hash
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
    }
    if let Some(hash) = fingerprint.strip_prefix("MD5:") {
        return hash.len() == 47
            && hash.split(':').count() == 16
            && hash
                .split(':')
                .all(|pair| pair.len() == 2 && pair.chars().all(|c| c.is_ascii_hexdigit()));
    }
    false
}

pub fn ask_host_keys(asker: HostKeyAsker) {
    *ASKER.write().unwrap_or_else(PoisonError::into_inner) = Some(asker);
}

fn asker() -> Option<HostKeyAsker> {
    ASKER.read().unwrap_or_else(PoisonError::into_inner).clone()
}

fn desk() -> Option<&'static Desk> {
    DESK.get_or_init(|| {
        open_desk()
            .inspect_err(|err| tracing::error!(error = ?err, context = "host key questions"))
            .ok()
    })
    .as_ref()
}

fn open_desk() -> std::io::Result<Desk> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let endpoint = listener.local_addr()?.to_string();
    let token = new_token()?;
    let expected = token.clone();
    std::thread::Builder::new()
        .name("host-key-desk".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let expected = expected.clone();
                let served = std::thread::Builder::new()
                    .name("host-key-question".into())
                    .spawn(move || serve(&stream, &expected, asker()));
                if let Err(err) = served {
                    tracing::error!(error = ?err, context = "host key question thread");
                }
            }
        })?;
    Ok(Desk { endpoint, token })
}

fn new_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (x, y)| diff | (x ^ y))
            == 0
}

fn serve(stream: &TcpStream, token: &str, asker: Option<HostKeyAsker>) {
    if let Err(err) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
        tracing::error!(error = ?err, context = "host key question timeout");
        return;
    }
    let mut line = String::new();
    if BufReader::new(stream.take(MAX_REQUEST))
        .read_line(&mut line)
        .is_err()
    {
        return;
    }
    let asked = match serde_json::from_str(&line) {
        Ok(Incoming::Host(asked)) => asked,
        Ok(Incoming::Repository(request)) => {
            let login = LOGINS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&request.token)
                .cloned();
            let Some(login) = login else { return };
            let answer = match read_prompt(&request.prompt) {
                SshPrompt::UnknownHost(question)
                    if login.interactive && ask(stream, asker, &question) =>
                {
                    Some("yes\n".to_owned())
                }
                SshPrompt::Other
                    if request
                        .prompt
                        .to_ascii_lowercase()
                        .starts_with("enter passphrase for key ") =>
                {
                    login.passphrase.map(|pass| format!("{pass}\n"))
                }
                _ => None,
            };
            let mut writer = stream;
            if let Err(err) = writer.set_nonblocking(false).and_then(|()| {
                serde_json::to_writer(&mut writer, &answer).map_err(std::io::Error::other)
            }) {
                tracing::error!(error = ?err, context = "answering the SSH repository prompt");
            }
            return;
        }
        Err(err) => {
            tracing::error!(error = ?err, context = "a host key question");
            return;
        }
    };
    if !same(&asked.token, token) {
        tracing::warn!("a host key question without the token");
        return;
    }
    let accepted = ask(stream, asker, &asked.question);
    let answer: &[u8] = if accepted { b"yes\n" } else { b"no\n" };
    let mut writer = stream;
    if let Err(err) = writer
        .set_nonblocking(false)
        .and_then(|()| writer.write_all(answer))
    {
        tracing::error!(error = ?err, context = "answering a host key question");
    }
}

fn ask(stream: &TcpStream, asker: Option<HostKeyAsker>, question: &HostKeyQuestion) -> bool {
    let accepted = match asker {
        Some(ask) if stream.set_nonblocking(true).is_ok() => {
            let gone = || {
                let mut probe = [0u8; 1];
                match stream.peek(&mut probe) {
                    Ok(0) => true,
                    Ok(_) => false,
                    Err(err) => err.kind() != std::io::ErrorKind::WouldBlock,
                }
            };
            ask(question, &gone)
        }
        _ => false,
    };
    tracing::info!(
        host = %question.host,
        key_type = %question.key_type,
        fingerprint = %question.fingerprint,
        accepted,
        "ssh host key question"
    );
    accepted
}

pub(crate) fn ask_app(question: &HostKeyQuestion) -> bool {
    let (Ok(endpoint), Ok(token)) = (std::env::var(ENDPOINT_VAR), std::env::var(TOKEN_VAR)) else {
        return false;
    };
    let Ok(address) = endpoint.parse::<SocketAddr>() else {
        return false;
    };
    if !address.ip().is_loopback() {
        return false;
    }
    let asked = Asked {
        token,
        question: question.clone(),
    };
    let Ok(mut request) = serde_json::to_string(&asked) else {
        return false;
    };
    request.push('\n');
    let answered = || -> std::io::Result<String> {
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(HOST_KEY_ASK_LIMIT + Duration::from_secs(30)))?;
        stream.write_all(request.as_bytes())?;
        let mut answer = String::new();
        BufReader::new(stream.take(16)).read_line(&mut answer)?;
        Ok(answer)
    };
    answered().is_ok_and(|answer| answer.trim_end() == "yes")
}

fn desk_env() -> Option<Vec<(String, String)>> {
    asker()?;
    let desk = desk()?;
    Some(vec![
        (ENDPOINT_VAR.to_owned(), desk.endpoint.clone()),
        (TOKEN_VAR.to_owned(), desk.token.clone()),
    ])
}

pub(crate) fn user_ssh(own_ssh_command: bool) -> (Option<&'static str>, Vec<(String, String)>) {
    let batch = (!own_ssh_command).then_some(crate::pulse::BATCH_SSH);
    let refusal = crate::pulse::askpass_refusal(own_ssh_command);
    let Some(mut env) = desk_env() else {
        return (batch, refusal);
    };
    let helper = crate::pulse::askpass_refusal(true);
    if helper.is_empty() {
        env.extend(refusal);
        return (batch, env);
    }
    env.extend(helper);
    ((!own_ssh_command).then_some(ASKING_SSH), env)
}

pub fn repository_ssh(
    command: &mut std::process::Command,
    credential: Option<&crate::Credential>,
    background: bool,
) -> std::io::Result<Option<RepositoryAskpass>> {
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("COGIT_CREDENTIAL_") {
            command.env_remove(name);
        }
    }
    command
        .env_remove("COGIT_SSH_PASSPHRASE")
        .env_remove(ENDPOINT_VAR)
        .env_remove(TOKEN_VAR)
        .env_remove(REPOSITORY_ASKPASS)
        .env_remove(crate::pulse::ASKPASS_REFUSAL_VAR);
    command.args(["-o", "ConnectTimeout=20", "-o", "ServerAliveInterval=15"]);
    if let Some(key) = credential.and_then(|credential| credential.key.as_deref()) {
        command
            .arg("-i")
            .arg(key)
            .args(["-o", "IdentitiesOnly=yes"]);
    }
    if background {
        command.args(["-o", "StrictHostKeyChecking=yes"]);
        if !credential.is_some_and(|key| key.key.is_some() && !key.secret.is_empty()) {
            command.args(["-o", "BatchMode=yes"]);
            return Ok(None);
        }
    } else {
        command.args(["-o", "StrictHostKeyChecking=ask"]);
    }
    let endpoint = desk()
        .ok_or_else(|| std::io::Error::other("Cogit could not start its SSH askpass service."))?
        .endpoint
        .clone();
    let token = new_token()?;
    let mut file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(
        &mut file,
        &Desk {
            endpoint,
            token: token.clone(),
        },
    )?;
    file.flush()?;
    let session = RepositoryAskpass { file, token };
    command
        .env("SSH_ASKPASS", std::env::current_exe()?)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env(REPOSITORY_ASKPASS, session.file.path());
    LOGINS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            session.token.clone(),
            Login {
                interactive: !background,
                passphrase: credential
                    .filter(|key| key.key.is_some() && !key.secret.is_empty())
                    .map(|key| key.secret.clone()),
            },
        );
    Ok(Some(session))
}

pub(crate) fn repository_answer(file: &std::path::Path, prompt: &str) -> Option<String> {
    let desk: Desk =
        serde_json::from_reader(std::fs::File::open(file).ok()?.take(MAX_REQUEST)).ok()?;
    let address = desk.endpoint.parse::<SocketAddr>().ok()?;
    if !address.ip().is_loopback() {
        return None;
    }
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).ok()?;
    stream
        .set_read_timeout(Some(HOST_KEY_ASK_LIMIT + Duration::from_secs(30)))
        .ok()?;
    serde_json::to_writer(
        &mut stream,
        &RepositoryPrompt {
            token: desk.token,
            prompt: prompt.into(),
        },
    )
    .ok()?;
    stream.write_all(b"\n").ok()?;
    serde_json::from_reader(stream.take(MAX_REQUEST)).ok()?
}

#[must_use]
pub fn host_key_summary(stderr: &str) -> Option<String> {
    if stderr.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        || stderr.contains("POSSIBLE DNS SPOOFING DETECTED")
    {
        let host = stderr.lines().find_map(|line| {
            line.trim()
                .strip_prefix("Host key for ")?
                .split_once(" has changed")
                .map(|(host, _)| host)
        });
        let remove = match host {
            Some(host) if host.contains('[') => format!("ssh-keygen -R \"{host}\""),
            Some(host) => format!("ssh-keygen -R {host}"),
            None => "ssh-keygen -R <host>".to_owned(),
        };
        return Some(format!(
            "The server's SSH host key has changed: the connection may be intercepted, or the server was reinstalled.\nCogit never accepts a changed key. Confirm the new fingerprint with the server's owner, then run {remove}"
        ));
    }
    stderr.contains("Host key verification failed.").then(|| {
        "The server's SSH host key was not accepted, so nothing was sent or received.".to_owned()
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const GITHUB_9: &str = "The authenticity of host 'github.com (140.82.121.4)' can't be established.\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
    const GITLAB_8: &str = "The authenticity of host 'gitlab.com (172.65.251.78)' can't be established.\nECDSA key fingerprint is SHA256:HbW3g8zUjNSksFbqTiUWPWg2Bq1x8xdGUrliXFzSnUw.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
    const BITBUCKET_7: &str = "The authenticity of host 'bitbucket.org (104.192.141.1)' can't be established.\nRSA key fingerprint is SHA256:zzXQOXSRBEiUtuE8AikJYKwbHaxvSc0ojez9YXaGp1A.\nAre you sure you want to continue connecting (yes/no)? ";
    const WINDOWS_PORT: &str = "The authenticity of host '[git.example.com]:2222 ([203.0.113.7]:2222)' can't be established.\r\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\r\nThis host key is known by the following other names/addresses:\r\n    ~/.ssh/known_hosts:4: [198.51.100.2]:2222\r\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
    const OTHER_TYPE: &str = "The authenticity of host 'h.example (192.0.2.1)' can't be established\nbut keys of different type are already known for this host.\nECDSA key fingerprint is SHA256:HbW3g8zUjNSksFbqTiUWPWg2Bq1x8xdGUrliXFzSnUw.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
    const PROXY_MD5_ART: &str = "The authenticity of host 'h.example (<no hostip for proxy command>)' can't be established.\nRSA key fingerprint is MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48.\n+---[RSA 3072]----+\n|        .o+.     |\n+----[MD5]--------+\nNo matching host key fingerprint found in DNS.\nAre you sure you want to continue connecting (yes/no)? ";
    const IP_CHANGED: &str = "Warning: the ECDSA host key for 'h.example' differs from the key for the IP address '192.0.2.9'\nOffending key for IP in /home/ann/.ssh/known_hosts:5\nMatching host key in /home/ann/.ssh/known_hosts:6\nAre you sure you want to continue connecting (yes/no)? ";
    const IDENTIFICATION_CHANGED: &str = "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\nIT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!\nThe fingerprint for the ED25519 key sent by the remote host is\nSHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

    fn unknown(prompt: &str) -> HostKeyQuestion {
        match read_prompt(prompt) {
            SshPrompt::UnknownHost(question) => question,
            other => panic!("{other:?} for {prompt}"),
        }
    }

    #[test]
    fn an_unknown_host_is_read_from_every_openssh_wording() {
        let github = unknown(GITHUB_9);
        assert_eq!(github.host, "github.com");
        assert_eq!(github.address.as_deref(), Some("140.82.121.4"));
        assert_eq!(github.key_type, "ED25519");
        assert_eq!(
            github.fingerprint,
            "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU"
        );
        assert!(!github.other_types_known);

        assert_eq!(unknown(GITLAB_8).key_type, "ECDSA");
        let bitbucket = unknown(BITBUCKET_7);
        assert_eq!(bitbucket.host, "bitbucket.org");
        assert_eq!(bitbucket.key_type, "RSA");

        let port = unknown(WINDOWS_PORT);
        assert_eq!(port.host, "[git.example.com]:2222");
        assert_eq!(port.address.as_deref(), Some("[203.0.113.7]:2222"));

        assert!(unknown(OTHER_TYPE).other_types_known);

        let proxy = unknown(PROXY_MD5_ART);
        assert_eq!(proxy.host, "h.example");
        assert_eq!(proxy.address, None);
        assert_eq!(
            proxy.fingerprint,
            "MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48"
        );
    }

    #[test]
    fn a_changed_key_is_never_an_unknown_host() {
        assert_eq!(read_prompt(IP_CHANGED), SshPrompt::ChangedHost);
        assert_eq!(read_prompt(IDENTIFICATION_CHANGED), SshPrompt::ChangedHost);
    }

    #[test]
    fn anything_else_is_another_question() {
        for prompt in [
            "Enter passphrase for key '/home/ann/.ssh/id_ed25519': ",
            "git@h.example's password: ",
            "Accept updated hostkeys? (yes/no): ",
            "Allow use of key /k/id? Key fingerprint SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.",
            "",
            "The authenticity of host 'h.example (192.0.2.1)' can't be established.\nAre you sure you want to continue connecting (yes/no)? ",
            "The authenticity of host 'h.example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:short.\nAre you sure you want to continue connecting (yes/no)? ",
            "The authenticity of host 'h.example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nRSA key fingerprint is SHA256:zzXQOXSRBEiUtuE8AikJYKwbHaxvSc0ojez9YXaGp1A.\nAre you sure you want to continue connecting (yes/no)? ",
            "The authenticity of host 'h.example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nType the password: ",
            "The authenticity of host 'h.ex\u{7}ample' can't be established.\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nAre you sure you want to continue connecting (yes/no)? ",
        ] {
            assert_eq!(read_prompt(prompt), SshPrompt::Other, "{prompt}");
        }
    }

    #[test]
    fn a_question_reaches_the_asker_only_with_the_token() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let heard = Arc::clone(&seen);
        let ask: HostKeyAsker =
            Arc::new(move |question: &HostKeyQuestion, _: &dyn Fn() -> bool| {
                heard.lock().unwrap().push(question.host.clone());
                question.host == "github.com"
            });
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let endpoint = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(&stream, "t0ken", Some(Arc::clone(&ask)));
            }
        });
        let send = |token: &str, question: &HostKeyQuestion| {
            let mut stream = TcpStream::connect(endpoint).unwrap();
            let asked = Asked {
                token: token.to_owned(),
                question: question.clone(),
            };
            let mut line = serde_json::to_string(&asked).unwrap();
            line.push('\n');
            stream.write_all(line.as_bytes()).unwrap();
            let mut answer = String::new();
            let _ = BufReader::new(stream).read_line(&mut answer);
            answer
        };

        assert_eq!(send("t0ken", &unknown(GITHUB_9)), "yes\n");
        assert_eq!(send("t0ken", &unknown(GITLAB_8)), "no\n");
        assert_eq!(send("forged", &unknown(GITHUB_9)), "");
        assert_eq!(*seen.lock().unwrap(), ["github.com", "gitlab.com"]);
    }

    #[test]
    fn a_changed_key_explains_itself() {
        let stderr = format!(
            "{IDENTIFICATION_CHANGED}\nHost key for [git.example.com]:2222 has changed and you have requested strict checking.\nHost key verification failed.\nfatal: Could not read from remote repository."
        );
        let summary = host_key_summary(&stderr).unwrap();
        assert!(summary.contains("has changed"), "{summary}");
        assert!(
            summary.ends_with("ssh-keygen -R \"[git.example.com]:2222\""),
            "{summary}"
        );
        assert!(
            host_key_summary("Host key verification failed.\nfatal: Could not read")
                .unwrap()
                .contains("not accepted")
        );
        assert_eq!(host_key_summary("fatal: repository not found"), None);
    }

    #[test]
    fn a_repository_login_keeps_secrets_out_of_the_ssh_environment_and_revokes_askpass() {
        let credential = crate::Credential {
            key: Some("/keys/my key".into()),
            secret: "session-passphrase".into(),
            username: None,
        };
        for background in [false, true] {
            let mut command = std::process::Command::new("ssh");
            let session = repository_ssh(&mut command, Some(&credential), background)
                .unwrap()
                .unwrap();
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy())
                .collect();
            let values: Vec<_> = command
                .get_envs()
                .filter_map(|(_, value)| value)
                .map(|v| v.to_string_lossy())
                .collect();
            assert!(args.iter().any(|arg| arg == "/keys/my key"));
            assert!(args.iter().any(|arg| arg
                == if background {
                    "StrictHostKeyChecking=yes"
                } else {
                    "StrictHostKeyChecking=ask"
                }));
            assert!(args.iter().chain(&values).all(
                |value| !value.contains(&credential.secret) && !value.contains(&session.token)
            ));
            let file = session.file.path();
            assert!(
                !std::fs::read_to_string(file)
                    .unwrap()
                    .contains(&credential.secret)
            );
            assert_eq!(
                repository_answer(file, "Enter passphrase for key '/keys/my key': ").as_deref(),
                Some("session-passphrase\n")
            );
            assert!(repository_answer(file, "git@host's password: ").is_none());
            assert!(repository_answer(file, IDENTIFICATION_CHANGED).is_none());
            if background {
                assert!(repository_answer(file, GITHUB_9).is_none());
            }
            let retained = tempfile::NamedTempFile::new().unwrap();
            std::fs::copy(file, retained.path()).unwrap();
            drop(session);
            assert!(
                repository_answer(retained.path(), "Enter passphrase for key '/keys/my key': ")
                    .is_none()
            );
        }
    }
}
