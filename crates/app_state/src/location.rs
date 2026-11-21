//! Where a repository's files live: the one place that classifies a root by backend (M15).

use std::fmt::{self, Write as _};
use std::net::Ipv6Addr;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SshHost {
    pub user: Option<String>,
    pub hostname: String,
    pub port: Option<u16>,
}

impl SshHost {
    fn parse(authority: &str) -> Option<Self> {
        let (user, server) = match authority.split_once('@') {
            Some((user, server)) if valid_name(user) => (Some(user.to_owned()), server),
            Some(_) => return None,
            None => (None, authority),
        };
        let (hostname, port) = if let Some(literal) = server.strip_prefix('[') {
            let (addr, suffix) = literal.split_once(']')?;
            let port = if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':')?)
            };
            (addr.parse::<Ipv6Addr>().ok()?.to_string(), port)
        } else {
            let (name, port) = server
                .split_once(':')
                .map_or((server, None), |(name, port)| (name, Some(port)));
            if !valid_name(name) {
                return None;
            }
            (name.to_ascii_lowercase(), port)
        };
        let port = match port {
            Some(text) if text.bytes().all(|c| c.is_ascii_digit()) => {
                Some(text.parse::<u16>().ok().filter(|p| *p != 0)?)
            }
            Some(_) => return None,
            None => None,
        };
        Some(Self {
            user,
            hostname,
            port,
        })
    }

    pub fn repository_url(&self, path: &str) -> String {
        let mut url = format!("ssh://{self}");
        if !path.starts_with('/') {
            url.push_str("/~/");
        }
        for byte in path.bytes() {
            if byte.is_ascii_alphanumeric() || b"/._-".contains(&byte) {
                url.push(char::from(byte));
            } else {
                let _ = write!(url, "%{byte:02X}");
            }
        }
        url
    }
}

impl fmt::Display for SshHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(user) = &self.user {
            write!(f, "{user}@")?;
        }
        if self.hostname.contains(':') {
            write!(f, "[{}]", self.hostname)?;
        } else {
            f.write_str(&self.hostname)?;
        }
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }
        Ok(())
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

fn decode_path(encoded: &str) -> Option<String> {
    let mut bytes = encoded.bytes();
    let mut path = Vec::with_capacity(encoded.len());
    while let Some(byte) = bytes.next() {
        path.push(if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()?
        } else {
            byte
        });
    }
    String::from_utf8(path).ok()
}

/// One window lists repositories of several backends at once; every root names its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoLocation {
    Local,
    /// `path` is the root as the distribution sees it: absolute, `/`-separated.
    Wsl {
        distro: String,
        path: String,
    },
    Ssh {
        host: SshHost,
        path: String,
    },
}

impl RepoLocation {
    pub fn of(root: &Path) -> Self {
        Self::checked(root).unwrap_or(Self::Local)
    }

    pub fn checked(root: &Path) -> Result<Self, &'static str> {
        const INVALID: &str = "Invalid SSH repository address. Use ssh://user@host[:port]/path or user@host:path. Passwords in addresses are unsupported.";
        let text = root.to_string_lossy();
        let uri = text
            .get(..6)
            .is_some_and(|p| p.eq_ignore_ascii_case("ssh://"));
        if let Some(location) = Self::ssh(&text, uri) {
            return Ok(location);
        }
        if uri
            || text.split_once('@').is_some_and(|(login, rest)| {
                login.contains(':') && !login.contains(['/', '\\']) && rest.contains(':')
            })
        {
            return Err(INVALID);
        }
        Ok(Self::wsl(&text.replace('\\', "/")).unwrap_or(Self::Local))
    }

    /// Text shown next to a repository when the list mixes backends.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Local => "local".to_owned(),
            Self::Wsl { distro, .. } => format!("WSL: {distro}"),
            Self::Ssh { host, .. } => format!("SSH: {host}"),
        }
    }

    fn ssh(text: &str, uri: bool) -> Option<Self> {
        let (host, path) = if uri {
            let (authority, encoded) = text[6..].split_once('/')?;
            if encoded.contains(['?', '#']) {
                return None;
            }
            let path = match encoded.strip_prefix("~/") {
                Some(relative) => decode_path(relative)?,
                None if encoded == "~" => String::new(),
                None => format!("/{}", decode_path(encoded)?),
            };
            (SshHost::parse(authority)?, path)
        } else {
            let (user, rest) = text.split_once('@')?;
            if !valid_name(user) {
                return None;
            }
            let (server, path) = if rest.starts_with('[') {
                let end = rest.find("]:")? + 1;
                (&rest[..end], &rest[end + 1..])
            } else {
                rest.split_once(':')?
            };
            if path.is_empty() {
                return None;
            }
            let path = match path.strip_prefix('~') {
                Some("") => "",
                Some(suffix) => suffix.strip_prefix('/')?,
                None => path,
            };
            (
                SshHost::parse(&format!("{user}@{server}"))?,
                path.to_owned(),
            )
        };
        if path.chars().any(char::is_control) {
            return None;
        }
        let trimmed = path.trim_end_matches('/');
        let path = if trimmed.is_empty() {
            if path.starts_with('/') { "/" } else { "." }
        } else {
            trimmed
        };
        Some(Self::Ssh {
            host,
            path: path.to_owned(),
        })
    }

    fn wsl(text: &str) -> Option<Self> {
        // `canonicalize` on Windows yields the verbatim form `//?/UNC/wsl.localhost/…`.
        let unc = text
            .strip_prefix("//?/UNC/")
            .or_else(|| text.strip_prefix("//"))?;
        let mut parts = unc.splitn(3, '/');
        let host = parts.next()?;
        if !host.eq_ignore_ascii_case("wsl.localhost") && !host.eq_ignore_ascii_case("wsl$") {
            return None;
        }
        let distro = parts.next().filter(|d| !d.is_empty())?;
        let rest = parts.next().unwrap_or_default().trim_end_matches('/');
        Some(Self::Wsl {
            distro: distro.to_owned(),
            path: format!("/{rest}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RepoLocation;
    use std::path::Path;

    fn wsl(distro: &str, path: &str) -> RepoLocation {
        RepoLocation::Wsl {
            distro: distro.into(),
            path: path.into(),
        }
    }

    #[test]
    fn recognizes_every_spelling_of_a_wsl_root() {
        let cases = [
            r"\\wsl.localhost\Ubuntu\home\brano\x",
            r"\\wsl$\Ubuntu\home\brano\x\",
            r"\\?\UNC\wsl.localhost\Ubuntu\home\brano\x",
            "//WSL.LOCALHOST/Ubuntu/home/brano/x",
        ];
        for case in cases {
            assert_eq!(
                RepoLocation::of(Path::new(case)),
                wsl("Ubuntu", "/home/brano/x"),
                "{case}"
            );
        }
        assert_eq!(
            RepoLocation::of(Path::new(r"\\wsl$\Debian")),
            wsl("Debian", "/")
        );
    }

    #[test]
    fn recognizes_an_ssh_root() {
        assert_eq!(
            RepoLocation::of(Path::new("ssh://me@build:2222/srv/x/")),
            RepoLocation::Ssh {
                host: super::SshHost {
                    user: Some("me".into()),
                    hostname: "build".into(),
                    port: Some(2222),
                },
                path: "/srv/x".into()
            }
        );
        assert_eq!(
            RepoLocation::of(Path::new("ssh:///srv/x")),
            RepoLocation::Local
        );
    }

    #[test]
    fn everything_else_is_local() {
        for case in [
            r"C:\src\x",
            r"\\server\share\x",
            r"\\?\C:\src\x",
            r"\\wsl$\",
            "/home/x",
        ] {
            assert_eq!(
                RepoLocation::of(Path::new(case)),
                RepoLocation::Local,
                "{case}"
            );
        }
    }

    #[test]
    fn scp_and_url_paths_share_the_same_host_and_preserve_home_relative_paths() {
        for (url, authority, remote) in [
            ("ann@Build:~/my repo/", "ann@build", "my repo"),
            ("ann@Build:~", "ann@build", "."),
            ("ann@build:notes:today", "ann@build", "notes:today"),
            ("ann@build:/srv/project", "ann@build", "/srv/project"),
            (
                "ann@[2001:db8::1]:/srv/project",
                "ann@[2001:db8::1]",
                "/srv/project",
            ),
            (
                "ssh://ann@[2001:db8::1]:2200/~/my%20repo",
                "ann@[2001:db8::1]:2200",
                "my repo",
            ),
            ("ssh://BUILD/", "build", "/"),
            ("ssh://build/%7E/a%23b%25c", "build", "/~/a#b%c"),
            ("ann@build:literal%20name", "ann@build", "literal%20name"),
        ] {
            let RepoLocation::Ssh { host, path } = RepoLocation::of(Path::new(url)) else {
                panic!("expected SSH for {url}");
            };
            assert_eq!(host.to_string(), authority);
            assert_eq!(path, remote);
            assert_eq!(
                RepoLocation::of(Path::new(&host.repository_url(&path))),
                RepoLocation::Ssh { host, path }
            );
        }
    }

    #[test]
    fn rejects_ambiguous_local_paths_and_malformed_ssh_authorities() {
        for input in [
            "D:relative",
            "file:name",
            "relative/path:entry",
            "https://host/repo",
            "ssh://-target/x",
            "ssh://user:password@host/x",
            "ssh://user@/x",
            "ssh://host:0/x",
            "ssh://host:65536/x",
            "ssh://host:port/x",
            "ssh://[bad:ipv6]/x",
            "ssh://[::1]junk/x",
            "ssh://a;b/x",
            "ssh://foo%20bar/x",
            "me@host:-x\ny",
            "ssh://a/x%00y",
            "ssh://2001:db8::1/x",
            "ssh://host/x%2",
            "ssh://host/x%QQ",
        ] {
            assert_eq!(
                RepoLocation::of(Path::new(input)),
                RepoLocation::Local,
                "{input}"
            );
        }
    }

    #[test]
    fn invalid_ssh_input_never_reaches_local_open_or_appears_in_an_error() {
        for value in [
            "ssh://ann:private-value@build/repo",
            "SSH://ann:private-value@build/repo",
            "ann:private-value@build:repo",
            "ssh://ann@build:invalid/repo",
        ] {
            let reason = RepoLocation::checked(Path::new(value)).unwrap_err();
            assert!(reason.contains("SSH"));
            assert!(!reason.contains("private-value"));
        }
        assert_eq!(
            RepoLocation::checked(Path::new("C:/repo@host:extra")),
            Ok(RepoLocation::Local)
        );
    }

    #[test]
    fn labels_name_the_backend() {
        assert_eq!(RepoLocation::Local.label(), "local");
        assert_eq!(wsl("Ubuntu", "/x").label(), "WSL: Ubuntu");
        assert_eq!(
            RepoLocation::of(Path::new("ssh://build/x")).label(),
            "SSH: build"
        );
    }
}
