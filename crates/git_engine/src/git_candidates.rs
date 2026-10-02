use crate::git_probe::{GitProbe, probe_git};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The oldest git the test fixtures are written against; an older one still runs, with a
/// warning (R-700).
pub const MIN_GIT: (u32, u32) = (2, 45);

/// `2.41.0` is older than `MIN_GIT`; a version that cannot be read is not.
#[must_use]
pub fn is_below_min_git(version: &str) -> bool {
    crate::maintenance::release_of(&format!("git version {version}"))
        .is_some_and(|release| release < MIN_GIT)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitCandidate {
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Unix,
}

impl Platform {
    pub const CURRENT: Self = if cfg!(windows) {
        Self::Windows
    } else {
        Self::Unix
    };
}

/// Everything the search asks of the machine, so a test can answer instead.
pub trait Lookup {
    fn var(&self, name: &str) -> Option<PathBuf>;
    fn is_file(&self, path: &Path) -> bool;
    /// Sub-folders of `dir`, any order.
    fn child_dirs(&self, dir: &Path) -> Vec<PathBuf>;
    /// `InstallPath` of Git for Windows under `HKLM` or `HKCU`.
    fn install_path(&self, machine_wide: bool) -> Option<PathBuf>;
    fn canonical(&self, path: &Path) -> PathBuf;
}

const UNIX: &[&str] = &[
    "/usr/bin/git",
    "/usr/local/bin/git",
    "/opt/homebrew/bin/git",
    "/opt/local/bin/git",
    "/snap/bin/git",
    "/Library/Developer/CommandLineTools/usr/bin/git",
];

/// Where a git could be, best first; nothing is checked for existence here.
#[must_use]
pub fn candidate_paths(platform: Platform, lookup: &impl Lookup) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if platform == Platform::Unix {
        found.extend(UNIX.iter().map(PathBuf::from));
        if let Some(home) = lookup.var("HOME") {
            found.push(home.join(".nix-profile/bin/git"));
        }
        return found;
    }
    for machine_wide in [true, false] {
        if let Some(root) = lookup.install_path(machine_wide) {
            found.push(root.join("cmd").join("git.exe"));
        }
    }
    let under = |name: &str, rest: &[&str]| {
        lookup
            .var(name)
            .map(|base| rest.iter().fold(base, |path, part| path.join(part)))
    };
    let fixed = [
        under("ProgramFiles", &["Git", "cmd", "git.exe"]),
        under("ProgramFiles", &["Git", "bin", "git.exe"]),
        under("ProgramFiles(x86)", &["Git", "cmd", "git.exe"]),
        under("LOCALAPPDATA", &["Programs", "Git", "cmd", "git.exe"]),
        under(
            "USERPROFILE",
            &["scoop", "apps", "git", "current", "cmd", "git.exe"],
        ),
        under("USERPROFILE", &["scoop", "shims", "git.exe"]),
        under("ProgramData", &["chocolatey", "bin", "git.exe"]),
    ];
    found.extend(fixed.into_iter().flatten());
    if let Some(desktop) = under("LOCALAPPDATA", &["GitHubDesktop"]) {
        let mut apps: Vec<PathBuf> = lookup
            .child_dirs(&desktop)
            .into_iter()
            .filter(|dir| {
                dir.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("app-"))
            })
            .collect();
        apps.sort();
        apps.reverse();
        found.extend(apps.into_iter().map(|app| {
            ["resources", "app", "git", "cmd", "git.exe"]
                .iter()
                .fold(app, |path, part| path.join(part))
        }));
    }
    found
}

/// The candidates that exist and run, each once (by real path), in search order.
pub fn working_candidates(
    platform: Platform,
    lookup: &(impl Lookup + Sync),
    probe: &(impl Fn(&Path) -> GitProbe + Sync),
) -> Vec<GitCandidate> {
    let mut seen = std::collections::HashSet::new();
    let paths: Vec<PathBuf> = candidate_paths(platform, lookup)
        .into_iter()
        .filter(|path| lookup.is_file(path))
        .filter(|path| seen.insert(lookup.canonical(path)))
        .collect();
    std::thread::scope(|scope| {
        let probes: Vec<_> = paths
            .iter()
            .map(|path| scope.spawn(move || probe(path)))
            .collect();
        paths
            .iter()
            .zip(probes)
            .filter_map(|(path, probed)| {
                let probed = probed.join().ok()?;
                Some(GitCandidate {
                    path: path.display().to_string(),
                    version: probed.version.filter(|_| probed.valid)?,
                })
            })
            .collect()
    })
}

struct System;

impl Lookup for System {
    fn var(&self, name: &str) -> Option<PathBuf> {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn child_dirs(&self, dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(std::result::Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn install_path(&self, machine_wide: bool) -> Option<PathBuf> {
        if !cfg!(windows) {
            return None;
        }
        let hive = if machine_wide { "HKLM" } else { "HKCU" };
        let mut command = std::process::Command::new("reg");
        command
            .args([
                "query",
                &format!(r"{hive}\SOFTWARE\GitForWindows"),
                "/v",
                "InstallPath",
            ])
            .stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(crate::runner::CREATE_NO_WINDOW);
        }
        let output = command.output().ok().filter(|out| out.status.success())?;
        install_path_from_reg(&String::from_utf8_lossy(&output.stdout))
    }

    // Case-folded as well: Windows paths are, and one file must not be two entries.
    fn canonical(&self, path: &Path) -> PathBuf {
        let real = portable::real_path(path);
        if cfg!(windows) {
            PathBuf::from(real.to_string_lossy().to_lowercase())
        } else {
            real
        }
    }
}

/// The value of `InstallPath    REG_SZ    C:\Program Files\Git`.
fn install_path_from_reg(output: &str) -> Option<PathBuf> {
    let line = output.lines().find(|line| line.contains("InstallPath"))?;
    let (_, value) = line.split_once("REG_SZ")?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Every working git in the usual places. Runs the programs: call it off the UI thread.
#[must_use]
pub fn find_git_candidates() -> Vec<GitCandidate> {
    working_candidates(Platform::CURRENT, &System, &|path: &Path| {
        probe_git(&path.display().to_string(), Duration::from_secs(3))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[derive(Default)]
    struct Fake {
        vars: HashMap<&'static str, PathBuf>,
        files: HashSet<PathBuf>,
        dirs: HashMap<PathBuf, Vec<PathBuf>>,
        installs: [Option<PathBuf>; 2],
        aliases: HashMap<PathBuf, PathBuf>,
    }

    impl Lookup for Fake {
        fn var(&self, name: &str) -> Option<PathBuf> {
            self.vars.get(name).cloned()
        }
        fn is_file(&self, path: &Path) -> bool {
            self.files.contains(path)
        }
        fn child_dirs(&self, dir: &Path) -> Vec<PathBuf> {
            self.dirs.get(dir).cloned().unwrap_or_default()
        }
        fn install_path(&self, machine_wide: bool) -> Option<PathBuf> {
            self.installs[usize::from(!machine_wide)].clone()
        }
        fn canonical(&self, path: &Path) -> PathBuf {
            self.aliases
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.to_path_buf())
        }
    }

    fn ok(version: &str) -> GitProbe {
        GitProbe {
            valid: true,
            version: Some(version.to_owned()),
            error: None,
            older_than: None,
        }
    }

    fn bad() -> GitProbe {
        GitProbe {
            valid: false,
            version: None,
            error: Some("no".into()),
            older_than: None,
        }
    }

    fn join(base: &str, parts: &[&str]) -> PathBuf {
        parts
            .iter()
            .fold(PathBuf::from(base), |path, part| path.join(part))
    }

    #[test]
    fn windows_paths_follow_the_environment_with_spaces_and_cyrillic() {
        let mut fake = Fake::default();
        fake.vars
            .insert("ProgramFiles", PathBuf::from(r"C:\Program Files"));
        fake.vars
            .insert("USERPROFILE", PathBuf::from(r"C:\Users\Иван Петров"));
        let paths = candidate_paths(Platform::Windows, &fake);
        assert_eq!(
            paths,
            vec![
                join(r"C:\Program Files", &["Git", "cmd", "git.exe"]),
                join(r"C:\Program Files", &["Git", "bin", "git.exe"]),
                join(
                    r"C:\Users\Иван Петров",
                    &["scoop", "apps", "git", "current", "cmd", "git.exe"]
                ),
                join(r"C:\Users\Иван Петров", &["scoop", "shims", "git.exe"]),
            ]
        );
    }

    #[test]
    fn the_registry_install_comes_first_and_github_desktop_newest_first() {
        let mut fake = Fake::default();
        fake.installs[0] = Some(PathBuf::from(r"D:\Git"));
        fake.vars.insert("LOCALAPPDATA", PathBuf::from(r"C:\L"));
        let desktop = join(r"C:\L", &["GitHubDesktop"]);
        fake.dirs.insert(
            desktop.clone(),
            vec![
                desktop.join("app-3.4.1"),
                desktop.join("app-3.5.0"),
                desktop.join("other"),
            ],
        );
        let paths = candidate_paths(Platform::Windows, &fake);
        assert_eq!(paths[0], join(r"D:\Git", &["cmd", "git.exe"]));
        let desktops: Vec<_> = paths.iter().filter(|p| p.starts_with(&desktop)).collect();
        assert_eq!(desktops.len(), 2);
        assert!(desktops[0].starts_with(desktop.join("app-3.5.0")));
    }

    #[test]
    fn unix_lists_the_usual_places_and_the_nix_profile() {
        let mut fake = Fake::default();
        fake.vars.insert("HOME", PathBuf::from("/home/ann"));
        let paths = candidate_paths(Platform::Unix, &fake);
        assert_eq!(paths[0], Path::new("/usr/bin/git"));
        assert_eq!(
            paths.last().unwrap(),
            Path::new("/home/ann/.nix-profile/bin/git")
        );
        assert!(paths.contains(&PathBuf::from("/opt/homebrew/bin/git")));
    }

    #[test]
    fn only_existing_files_that_run_are_offered_once_by_real_path() {
        let mut fake = Fake::default();
        fake.vars.insert("HOME", PathBuf::from("/h"));
        for file in [
            "/usr/bin/git",
            "/usr/local/bin/git",
            "/snap/bin/git",
            "/opt/local/bin/git",
        ] {
            fake.files.insert(PathBuf::from(file));
        }
        fake.aliases.insert(
            PathBuf::from("/usr/local/bin/git"),
            PathBuf::from("/usr/bin/git"),
        );
        let found = working_candidates(Platform::Unix, &fake, &|path: &Path| {
            if path.ends_with("opt/local/bin/git") {
                bad()
            } else {
                ok("2.50.1")
            }
        });
        let paths: Vec<_> = found.iter().map(|c| c.path.replace('\\', "/")).collect();
        assert_eq!(paths, ["/usr/bin/git", "/snap/bin/git"]);
        assert_eq!(found[0].version, "2.50.1");
    }

    #[test]
    fn the_minimum_is_compared_by_release() {
        assert!(is_below_min_git("2.44.3"));
        assert!(is_below_min_git("1.9.0"));
        assert!(!is_below_min_git("2.45.0"));
        assert!(!is_below_min_git("2.51.0"));
        assert!(!is_below_min_git("garbage"));
    }

    #[test]
    fn the_install_path_is_read_from_reg_output() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\GitForWindows\r\n    InstallPath    REG_SZ    C:\\Program Files\\Git\r\n\r\n";
        assert_eq!(
            install_path_from_reg(out),
            Some(PathBuf::from(r"C:\Program Files\Git"))
        );
        assert_eq!(install_path_from_reg("nothing"), None);
    }
}
