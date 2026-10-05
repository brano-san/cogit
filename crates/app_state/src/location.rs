//! Where a repository's files live: the one place that classifies a root by backend (M15).

use std::path::Path;

/// One window lists repositories of several backends at once; every root names its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoLocation {
    Local,
    /// `path` is the root as the distribution sees it: absolute, `/`-separated.
    Wsl {
        distro: String,
        path: String,
    },
    /// Root spelled `ssh://[user@]host[:port]/path`; `host` keeps the user and port.
    Ssh {
        host: String,
        path: String,
    },
}

impl RepoLocation {
    pub fn of(root: &Path) -> Self {
        let text = root.to_string_lossy().replace('\\', "/");
        if let Some(rest) = text.strip_prefix("ssh://") {
            return Self::ssh(rest).unwrap_or(Self::Local);
        }
        Self::wsl(&text).unwrap_or(Self::Local)
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

    fn ssh(rest: &str) -> Option<Self> {
        let (host, path) = rest.split_once('/')?;
        (!host.is_empty()).then(|| Self::Ssh {
            host: host.to_owned(),
            path: format!("/{}", path.trim_end_matches('/')),
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
                host: "me@build:2222".into(),
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
    fn labels_name_the_backend() {
        assert_eq!(RepoLocation::Local.label(), "local");
        assert_eq!(wsl("Ubuntu", "/x").label(), "WSL: Ubuntu");
        assert_eq!(
            RepoLocation::of(Path::new("ssh://build/x")).label(),
            "SSH: build"
        );
    }
}
