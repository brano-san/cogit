use crate::runner::bare_git;
use crate::{GitError, RepoHandle, Result};

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LfsOp {
    /// `--local`: the filters go into this repository's config, not the user's.
    Install,
    Track {
        pattern: String,
    },
    Lock {
        paths: Vec<String>,
    },
    Unlock {
        paths: Vec<String>,
    },
    Prune,
}

/// `git lfs version`, or `None` when git has no `lfs` command to run.
#[must_use]
pub fn lfs_version() -> Option<String> {
    let output = bare_git(&["lfs", "version"]).ok()?;
    lfs_version_from(output.exit_code, &output.stdout)
}

#[must_use]
pub fn lfs_version_from(exit_code: Option<i32>, stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    (exit_code == Some(0) && !line.is_empty()).then(|| line.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsLock {
    pub path: String,
    pub owner: String,
}

#[derive(serde::Deserialize)]
struct RawLock {
    path: String,
    #[serde(default)]
    owner: Option<RawOwner>,
}

#[derive(serde::Deserialize)]
struct RawOwner {
    name: String,
}

pub(crate) fn parse_locks(json: &str) -> Vec<LfsLock> {
    serde_json::from_str::<Vec<RawLock>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|lock| LfsLock {
            path: lock.path,
            owner: lock.owner.map(|owner| owner.name).unwrap_or_default(),
        })
        .collect()
}

/// A file `.gitattributes` puts in Git LFS; `lock` is the owner from the local lock cache.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsFileState {
    pub path: String,
    pub lockable: bool,
    pub lock: Option<String>,
}

/// `git check-attr -z` prints `path NUL attr NUL value NUL` triples.
pub(crate) fn parse_lfs_attrs(out: &str) -> Vec<LfsFileState> {
    let fields: Vec<&str> = out.split('\0').collect();
    let mut found = Vec::new();
    let mut lockable = std::collections::HashSet::new();
    for [path, attr, value] in fields.as_chunks::<3>().0 {
        match (*attr, *value) {
            ("filter", "lfs") => found.push(LfsFileState {
                path: (*path).to_owned(),
                lockable: false,
                lock: None,
            }),
            ("lockable", "set") => {
                lockable.insert(*path);
            }
            _ => {}
        }
    }
    for state in &mut found {
        state.lockable = lockable.contains(state.path.as_str());
    }
    found
}

impl RepoHandle {
    /// Which of `paths` are in Git LFS, with locks from `git lfs locks --local`: no network,
    /// so it can run on every refresh. Without `git-lfs` the locks stay empty.
    pub fn lfs_file_states(&self, paths: &[String]) -> Result<Vec<LfsFileState>> {
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        let out = self.read_git_fed(
            &["check-attr", "-z", "--stdin", "filter", "lockable"],
            &input,
        )?;
        let mut states = parse_lfs_attrs(&out);
        if states.is_empty() {
            return Ok(states);
        }
        let locks = match self.read_git(&["lfs", "locks", "--local", "--json"]) {
            Ok(json) => parse_locks(&json),
            Err(err) => {
                tracing::debug!(error = ?err, context = "lfs locks --local");
                Vec::new()
            }
        };
        for state in &mut states {
            state.lock = locks
                .iter()
                .find(|lock| lock.path == state.path)
                .map(|lock| lock.owner.clone());
        }
        Ok(states)
    }

    /// `git lfs locks` asks the server; a file locked by someone else is read-only here.
    pub fn lfs_locks(&self) -> Result<Vec<LfsLock>> {
        Ok(parse_locks(&self.read_git(&["lfs", "locks", "--json"])?))
    }

    pub fn lfs_op(&self, op: &LfsOp) -> Result<()> {
        match op {
            LfsOp::Install => self.run_git(&["lfs", "install", "--local"]).map(drop),
            LfsOp::Track { pattern } => {
                let pattern = pattern.trim();
                if pattern.is_empty() || pattern.starts_with('-') {
                    return Err(GitError::InvalidState(format!(
                        "\"{pattern}\" is not a file pattern"
                    )));
                }
                self.run_git(&["lfs", "track", "--", pattern]).map(drop)
            }
            LfsOp::Lock { paths } => self.lfs_each("lock", paths),
            LfsOp::Unlock { paths } => self.lfs_each("unlock", paths),
            LfsOp::Prune => self.run_git(&["lfs", "prune"]).map(drop),
        }
    }

    /// One call per file: `git lfs lock` takes a single path.
    fn lfs_each(&self, command: &str, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState(format!(
                "choose a file to {command}"
            )));
        }
        paths
            .iter()
            .try_for_each(|path| self.run_git(&["lfs", command, "--", path]).map(drop))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attrs_keep_only_lfs_files() {
        let out = "a.psd\0filter\0lfs\0a.psd\0lockable\0set\0b.txt\0filter\0unspecified\0b.txt\0lockable\0set\0c.bin\0filter\0lfs\0c.bin\0lockable\0unspecified\0";
        let state = |path: &str, lockable| LfsFileState {
            path: path.into(),
            lockable,
            lock: None,
        };
        assert_eq!(
            parse_lfs_attrs(out),
            [state("a.psd", true), state("c.bin", false)]
        );
        assert!(parse_lfs_attrs("").is_empty());
    }

    #[test]
    fn locks_json_names_path_and_owner() {
        let json = r#"[{"id":"1","path":"art/a.psd","owner":{"name":"ann"},"locked_at":"x"}]"#;
        assert_eq!(
            parse_locks(json),
            [LfsLock {
                path: "art/a.psd".into(),
                owner: "ann".into()
            }]
        );
        assert!(parse_locks("[]").is_empty());
    }
}
