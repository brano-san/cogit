use crate::location::SshHost;
use git_engine::{Credential, GitError};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Read as _, Write};
use std::process::Command;
use std::sync::OnceLock;

const BOOTSTRAP: &str = include_str!("ssh_bootstrap.sh");
const VERSION: &str = env!("CARGO_PKG_VERSION");
static DIGEST: OnceLock<String> = OnceLock::new();

pub(crate) fn inside_agent() -> bool {
    std::env::var_os("COGIT_SSH_AGENT_SESSION").is_some()
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn command(
    host: &SshHost,
    bytes: &'static [u8],
    credential: Option<&Credential>,
    background: bool,
    askpass: &mut Option<git_engine::RepositoryAskpass>,
) -> Result<Command, GitError> {
    if bytes.is_empty() {
        return Err(GitError::Unavailable("This build of Cogit carries no Linux x86_64 agent. Build it with COGIT_AGENT_BIN or install a release build.".into()));
    }
    #[cfg(feature = "loopback")]
    let program = std::env::var_os("COGIT_TEST_SSH").unwrap_or_else(|| "ssh".into());
    #[cfg(not(feature = "loopback"))]
    let program = "ssh";
    let mut command = Command::new(program);
    *askpass = git_engine::repository_ssh(&mut command, credential, background)?;
    command.env("LC_ALL", "C");
    command.args([
        "-T",
        "-a",
        "-x",
        "-o",
        "ClearAllForwardings=yes",
        "-o",
        "ControlPath=none",
        "-o",
        "PasswordAuthentication=no",
        "-o",
        "KbdInteractiveAuthentication=no",
        "-o",
        "UpdateHostKeys=no",
        "-o",
        "VerifyHostKeyDNS=no",
    ]);
    if let Some(port) = host.port {
        command.arg("-p").arg(port.to_string());
    }
    if let Some(user) = &host.user {
        command.arg("-l").arg(user);
    }
    let checksum = DIGEST.get_or_init(|| digest(bytes));
    let length = bytes.len();
    command.arg("--").arg(&host.hostname).arg(format!(
        "sh -c '{BOOTSTRAP}' cogit-agent {VERSION} {checksum} {length}"
    ));
    Ok(command)
}

fn line(reader: &mut impl BufRead) -> Result<String, GitError> {
    let mut line = String::new();
    reader.take(512).read_line(&mut line)?;
    if !line.ends_with('\n') {
        return Err(GitError::Unavailable(
            "The SSH server did not complete the Cogit agent setup.".into(),
        ));
    }
    Ok(line.trim_end().to_owned())
}

pub(crate) fn bootstrap(
    output: &mut impl BufRead,
    input: &mut impl Write,
    bytes: &[u8],
) -> Result<(), GitError> {
    let platform = line(output)?;
    if platform != "Linux x86_64" {
        return Err(GitError::Unavailable(format!(
            "Unsupported SSH server platform '{platform}'. This build ships an agent for Linux x86_64 only."
        )));
    }
    let mut state = line(output)?;
    if state == "upload" {
        input.write_all(bytes)?;
        input.flush()?;
        state = line(output)?;
    }
    if state != "ready" {
        return Err(GitError::Unavailable(format!(
            "Unexpected response during SSH agent installation: {state}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn an_unsupported_server_never_receives_an_agent() {
        for platform in [
            "Linux aarch64",
            "Darwin x86_64",
            "Linux",
            "Linux x86_64 extra",
        ] {
            let mut received = Vec::new();
            let response = format!("{platform}\n");
            let failure =
                bootstrap(&mut Cursor::new(response), &mut received, b"agent").unwrap_err();
            assert!(failure.to_string().contains("Linux x86_64"));
            assert!(received.is_empty());
        }
    }

    #[test]
    fn the_cached_agent_receives_no_upload_or_protocol_bytes() {
        let mut received = Vec::new();
        let mut response = Cursor::new(b"Linux x86_64\nready\nprotocol");
        bootstrap(&mut response, &mut received, b"agent").unwrap();
        assert!(received.is_empty());
        assert_eq!(response.fill_buf().unwrap(), b"protocol");
    }

    #[test]
    fn upload_must_finish_and_be_verified_before_the_protocol_starts() {
        let mut received = Vec::new();
        bootstrap(
            &mut Cursor::new(b"Linux x86_64\nupload\nready\n"),
            &mut received,
            b"agent",
        )
        .unwrap();
        assert_eq!(received, b"agent");
        assert!(
            bootstrap(
                &mut Cursor::new(b"Linux x86_64\nupload\n"),
                &mut Vec::new(),
                b"agent"
            )
            .is_err()
        );
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_install_script_checks_the_cached_and_incoming_binary() {
        use std::process::Stdio;
        let home = tempfile::tempdir().unwrap();
        let bytes = b"printf launched";
        let installed = home
            .path()
            .join(".cache/cogit/agent")
            .join(VERSION)
            .join("cogit-agent");
        let run = |payload: &[u8]| {
            let mut child = Command::new("sh")
                .args([
                    "-c",
                    &format!("uname() {{ echo Linux x86_64; }}\n{BOOTSTRAP}"),
                    "test",
                    VERSION,
                    &digest(bytes),
                    &bytes.len().to_string(),
                ])
                .env("HOME", home.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(payload).unwrap();
            child.wait_with_output().unwrap()
        };
        let first = run(bytes);
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(first.stdout, b"Linux x86_64\nupload\nready\nlaunched");
        assert_eq!(std::fs::read(&installed).unwrap(), bytes);
        assert_eq!(run(&[]).stdout, b"Linux x86_64\nready\nlaunched");
        std::fs::write(&installed, b"bad cache").unwrap();
        let rejected = run(b"bad upload");
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("SHA-256"));
        assert_eq!(std::fs::read(&installed).unwrap(), b"bad cache");
        assert_eq!(run(bytes).stdout, first.stdout);
    }
}
