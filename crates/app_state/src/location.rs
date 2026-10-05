//! Where a repository's files live: the one place that tells a WSL path from a local one (M15).

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoLocation {
    Local,
    /// `path` is the root as the distribution sees it: absolute, `/`-separated.
    Wsl {
        distro: String,
        path: String,
    },
}

impl RepoLocation {
    pub fn of(root: &Path) -> Self {
        let text = root.to_string_lossy().replace('\\', "/");
        // `canonicalize` on Windows yields the verbatim form `//?/UNC/wsl.localhost/…`.
        let unc = text
            .strip_prefix("//?/UNC/")
            .or_else(|| text.strip_prefix("//"));
        let Some(unc) = unc else {
            return Self::Local;
        };
        let mut parts = unc.splitn(3, '/');
        let host = parts.next().unwrap_or_default();
        if !host.eq_ignore_ascii_case("wsl.localhost") && !host.eq_ignore_ascii_case("wsl$") {
            return Self::Local;
        }
        let Some(distro) = parts.next().filter(|d| !d.is_empty()) else {
            return Self::Local;
        };
        let rest = parts.next().unwrap_or_default().trim_end_matches('/');
        Self::Wsl {
            distro: distro.to_owned(),
            path: format!("/{rest}"),
        }
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
}
