//! Programmatic generation of temporary Git repositories for tests.
//!
//! Fixtures are built with the **system `git`**, not with `gix`: tests must check our
//! code against how Git actually behaves, not against what `gix` believes about it.
//!
//! Full inventory of required shapes: `doc/09-testing.md` section 4. Implemented in M9.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Fixed identity so commit OIDs are reproducible across machines.
pub const AUTHOR_NAME: &str = "Cogit Fixture";
pub const AUTHOR_EMAIL: &str = "fixture@cogit.test";
/// Fixed timestamp (2026-01-01T00:00:00Z) — snapshot tests depend on stable OIDs.
pub const BASE_TIMESTAMP: i64 = 1_767_225_600;

#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("git {args:?} failed with status {status:?}:\n{stderr}")]
    Git {
        args: Vec<String>,
        status: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FixtureError>;

/// A temporary repository. The directory is removed when this value is dropped,
/// including when a test panics.
#[derive(Debug)]
pub struct Fixture {
    dir: TempDir,
}

impl Fixture {
    /// Creates an empty initialised repository with a hermetic environment.
    pub fn init() -> Result<Self> {
        let dir = TempDir::new()?;
        let fixture = Self { dir };
        fixture.git(&["init", "--initial-branch=main"])?;
        fixture.git(&["config", "user.name", AUTHOR_NAME])?;
        fixture.git(&["config", "user.email", AUTHOR_EMAIL])?;
        // Explicit so the developer's own global config cannot change the outcome.
        fixture.git(&["config", "core.autocrlf", "false"])?;
        fixture.git(&["config", "commit.gpgsign", "false"])?;
        Ok(fixture)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    #[must_use]
    pub fn git_dir(&self) -> PathBuf {
        self.dir.path().join(".git")
    }

    /// Runs `git` in the fixture with global and system configuration neutralised.
    ///
    /// Without this isolation a developer's `.gitconfig` (autocrlf, hooks, templates)
    /// leaks into results and tests become machine-dependent.
    pub fn git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.dir.path())
            .env(
                "GIT_CONFIG_GLOBAL",
                self.dir.path().join("no-global-config"),
            )
            .env(
                "GIT_CONFIG_SYSTEM",
                self.dir.path().join("no-system-config"),
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
            .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
            .env("GIT_COMMITTER_NAME", AUTHOR_NAME)
            .env("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL)
            .env("LC_ALL", "C")
            .output()?;

        if !output.status.success() {
            return Err(FixtureError::Git {
                args: args.iter().map(|s| (*s).to_owned()).collect(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Writes a file and commits it with a deterministic timestamp.
    ///
    /// `index` advances the commit time so ordering is stable but distinct.
    pub fn commit_file(&self, index: i64, name: &str, contents: &str) -> Result<String> {
        std::fs::write(self.dir.path().join(name), contents)?;
        self.git(&["add", "--", name])?;

        let stamp = format!("{} +0000", BASE_TIMESTAMP + index * 60);
        let output = Command::new("git")
            .args(["commit", "-m", &format!("commit {index}")])
            .current_dir(self.dir.path())
            .env(
                "GIT_CONFIG_GLOBAL",
                self.dir.path().join("no-global-config"),
            )
            .env(
                "GIT_CONFIG_SYSTEM",
                self.dir.path().join("no-system-config"),
            )
            .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
            .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
            .env("GIT_COMMITTER_NAME", AUTHOR_NAME)
            .env("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL)
            .env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp)
            .env("LC_ALL", "C")
            .output()?;

        if !output.status.success() {
            return Err(FixtureError::Git {
                args: vec!["commit".to_owned()],
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(self.git(&["rev-parse", "HEAD"])?.trim().to_owned())
    }

    /// Resolves a revision to a full OID.
    pub fn oid(&self, rev: &str) -> Result<String> {
        Ok(self.git(&["rev-parse", rev])?.trim().to_owned())
    }
}

/// A repository with `n` commits in a single line.
pub fn linear(n: i64) -> Result<Fixture> {
    let fixture = Fixture::init()?;
    for i in 0..n {
        fixture.commit_file(i, &format!("file{i}.txt"), &format!("content {i}\n"))?;
    }
    Ok(fixture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_a_repository() {
        let fixture = Fixture::init().unwrap();
        assert!(fixture.git_dir().is_dir());
    }

    #[test]
    fn linear_history_has_the_requested_length() {
        let fixture = linear(5).unwrap();
        let log = fixture.git(&["rev-list", "--count", "HEAD"]).unwrap();
        assert_eq!(log.trim(), "5");
    }

    #[test]
    fn commit_oids_are_deterministic() {
        // Snapshot tests of the graph depend on this holding across runs and machines.
        let a = linear(3).unwrap().oid("HEAD").unwrap();
        let b = linear(3).unwrap().oid("HEAD").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_failing_git_command_reports_stderr() {
        let fixture = Fixture::init().unwrap();
        let err = fixture.git(&["rev-parse", "does-not-exist"]).unwrap_err();
        match err {
            FixtureError::Git { stderr, .. } => assert!(!stderr.is_empty()),
            other => panic!("expected a git error, got {other:?}"),
        }
    }
}
