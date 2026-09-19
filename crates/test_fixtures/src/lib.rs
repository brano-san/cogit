//! Programmatic generation of temporary Git repositories for tests.
//!
//! Fixtures are built with the **system `git`**, not with `gix`: tests must check our
//! code against how Git actually behaves, not against what `gix` believes about it.
//!
//! Full inventory of required shapes: `doc/09-testing.md` section 4.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Fixed identity so commit OIDs are reproducible across machines.
pub const AUTHOR_NAME: &str = "Cogit Fixture";
pub const AUTHOR_EMAIL: &str = "fixture@cogit.test";
/// Fixed timestamp (2026-01-01T00:00:00Z) — snapshot tests depend on stable OIDs.
pub const BASE_TIMESTAMP: i64 = 1_767_225_600;
/// Spacing between successive commit timestamps.
const STEP_SECONDS: i64 = 60;

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
    /// Extra directories kept alive for the lifetime of the fixture — a submodule
    /// source or a linked worktree lives outside the repository but must not be
    /// cleaned up before it.
    _aux: Vec<TempDir>,
}

impl Fixture {
    /// Creates an empty initialised repository with a hermetic environment.
    pub fn init() -> Result<Self> {
        let dir = TempDir::new()?;
        let fixture = Self {
            dir,
            _aux: Vec::new(),
        };
        fixture.git(&["init", "--initial-branch=main"])?;

        // Written as a file rather than through four `git config` calls: each call is
        // a process launch, and on Windows those dominate fixture setup time.
        std::fs::write(
            fixture.git_dir().join("config"),
            format!(
                "[core]\n\
                 \trepositoryformatversion = 0\n\
                 \tfilemode = false\n\
                 \tbare = false\n\
                 \tautocrlf = false\n\
                 \tsymlinks = false\n\
                 \tquotepath = false\n\
                 [user]\n\
                 \tname = {AUTHOR_NAME}\n\
                 \temail = {AUTHOR_EMAIL}\n\
                 [commit]\n\
                 \tgpgsign = false\n\
                 [gc]\n\
                 \tauto = 0\n"
            ),
        )?;
        Ok(fixture)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Location of the Git directory.
    ///
    /// A bare repository has no `.git` subdirectory — the root itself is the git dir.
    #[must_use]
    pub fn git_dir(&self) -> PathBuf {
        let nested = self.dir.path().join(".git");
        if nested.is_dir() {
            nested
        } else {
            self.dir.path().to_path_buf()
        }
    }

    /// Runs `git` in the fixture. Use for commands that do not create a commit.
    pub fn git(&self, args: &[&str]) -> Result<String> {
        self.run(args, None)
    }

    /// Runs a `git` command that creates a commit, pinning its timestamp.
    ///
    /// `index` positions the commit on the fixed timeline, so repeated runs of the same
    /// fixture produce byte-identical objects and therefore identical OIDs.
    pub fn git_at(&self, index: i64, args: &[&str]) -> Result<String> {
        self.run(args, Some(index))
    }

    fn run(&self, args: &[&str], date_index: Option<i64>) -> Result<String> {
        run_git(self.dir.path(), args, date_index)
    }

    /// Writes a file and commits it with a deterministic timestamp.
    pub fn commit_file(&self, index: i64, name: &str, contents: &str) -> Result<String> {
        let target = self.dir.path().join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        self.git(&["add", "--", name])?;
        self.git_at(index, &["commit", "-m", &format!("commit {index}")])?;
        self.oid("HEAD")
    }

    /// Commits whatever is already staged, with a deterministic timestamp.
    pub fn commit_staged(&self, index: i64, message: &str) -> Result<String> {
        self.git_at(index, &["commit", "-m", message])?;
        self.oid("HEAD")
    }

    /// Writes a file into the working tree without staging or committing it.
    pub fn write_file(&self, name: &str, contents: &str) -> Result<()> {
        let target = self.dir.path().join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        Ok(())
    }

    /// Creates a merge commit with a deterministic timestamp.
    pub fn merge(&self, index: i64, refs: &[&str], message: &str) -> Result<String> {
        let mut args = vec!["merge", "--no-edit", "-m", message];
        args.extend_from_slice(refs);
        // A two-parent merge would fast-forward without this; an octopus merge never
        // fast-forwards, and git rejects the flag combination, so only add it for two.
        if refs.len() == 1 {
            args.insert(1, "--no-ff");
        }
        self.git_at(index, &args)?;
        self.oid("HEAD")
    }

    /// Resolves a revision to a full OID.
    pub fn oid(&self, rev: &str) -> Result<String> {
        Ok(self.git(&["rev-parse", rev])?.trim().to_owned())
    }

    /// Surrenders the temporary directory so another fixture can keep it alive.
    fn into_temp_dir(self) -> TempDir {
        self.dir
    }

    /// Path in the form Git wants for a local URL: forward slashes on every platform.
    fn url_path(&self) -> String {
        self.dir.path().to_string_lossy().replace('\\', "/")
    }
}

/// Variables through which an outer Git run reaches into any `git` it spawns.
///
/// Git sets these for its hooks. If a fixture inherits them, `current_dir` is ignored
/// and every command lands in the **caller's** repository — a fixture running from a
/// pre-commit hook would commit into the real project. They are cleared, not overridden,
/// so an unset variable stays unset.
const AMBIENT_GIT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_INDEX_VERSION",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_PREFIX",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    // `-c key=value` reaches subprocesses through this one.
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    // Nothing interactive may open while tests run.
    "GIT_EDITOR",
    "GIT_SEQUENCE_EDITOR",
];

/// Builds a fully isolated `git` invocation.
///
/// The single place where isolation is configured, so it cannot drift between the
/// plain and the stdin-fed call sites.
fn git_command(cwd: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd)
        // Neutralise the developer's own configuration. Without this, a global
        // `core.autocrlf` or a commit template silently changes the fixtures and
        // tests become machine-dependent. The paths need not exist: Git treats a
        // missing config file as an empty one.
        .env("GIT_CONFIG_GLOBAL", cwd.join("no-global-config"))
        .env("GIT_CONFIG_SYSTEM", cwd.join("no-system-config"))
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
        .env("GIT_COMMITTER_NAME", AUTHOR_NAME)
        .env("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL)
        .env("LC_ALL", "C");

    for name in AMBIENT_GIT_VARS {
        cmd.env_remove(name);
    }
    cmd
}

/// The single place that launches `git`, so environment isolation cannot drift between
/// call sites. Free-standing because some fixtures run `git` outside any one repository.
fn run_git(cwd: &Path, args: &[&str], date_index: Option<i64>) -> Result<String> {
    let mut cmd = git_command(cwd);
    cmd.args(args);

    if let Some(index) = date_index {
        let stamp = format!("{} +0000", BASE_TIMESTAMP + index * STEP_SECONDS);
        cmd.env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(FixtureError::Git {
            args: args.iter().map(|s| (*s).to_owned()).collect(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// An initialised repository with no commits at all.
pub fn empty() -> Result<Fixture> {
    Fixture::init()
}

/// A bare repository: history without a working tree.
pub fn bare() -> Result<Fixture> {
    let source = linear(3)?;
    let dir = TempDir::new()?;
    let src = source.path().to_string_lossy().into_owned();
    // Cloning rather than `init --bare` keeps the deterministic history from `linear`.
    run_git(dir.path(), &["clone", "--bare", "--", &src, "."], None)?;
    Ok(Fixture {
        dir,
        _aux: Vec::new(),
    })
}

/// HEAD pointing straight at a commit instead of a branch.
pub fn detached_head() -> Result<Fixture> {
    let f = linear(3)?;
    f.git(&["switch", "--detach", "HEAD~1"])?;
    Ok(f)
}

/// A merge stopped halfway by a conflict, with all three stages left in the index.
pub fn conflicted() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "conflict.txt", "original line\n")?;

    f.git(&["switch", "-c", "dev"])?;
    f.commit_file(1, "conflict.txt", "changed by dev\n")?;

    f.git(&["switch", "main"])?;
    f.commit_file(2, "conflict.txt", "changed by main\n")?;

    // This merge is *expected* to fail — an unfinished merge is the whole point of the
    // fixture, so a non-zero exit here is success, not an error to propagate.
    let _ = f.git_at(3, &["merge", "--no-edit", "dev"]);
    Ok(f)
}

/// Files with every line-ending style, for exercising INV-08.
pub fn crlf_files() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.write_file("crlf.txt", "first\r\nsecond\r\nthird\r\n")?;
    f.write_file("lf.txt", "first\nsecond\nthird\n")?;
    f.write_file("mixed.txt", "crlf line\r\nlf line\nanother crlf\r\n")?;
    f.git(&["add", "--", "crlf.txt", "lf.txt", "mixed.txt"])?;
    f.commit_staged(0, "line ending samples")?;
    Ok(f)
}

/// A commit that renames a file without touching its content.
pub fn renames() -> Result<Fixture> {
    let f = Fixture::init()?;
    // Enough content that rename detection is unambiguous rather than a coin flip.
    let body = (0..20).map(|i| format!("line {i}\n")).collect::<String>();
    f.commit_file(0, "old-name.txt", &body)?;
    f.git(&["mv", "old-name.txt", "new-name.txt"])?;
    f.commit_staged(1, "rename old-name.txt to new-name.txt")?;
    Ok(f)
}

/// A commit whose only change is the executable bit.
pub fn filemode_change() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "script.sh", "#!/bin/sh\necho hello\n")?;
    // Windows does not carry the executable bit in the working tree, so the mode is
    // set directly in the index. This is also how Git itself records the change.
    f.git(&["update-index", "--chmod=+x", "script.sh"])?;
    f.commit_staged(1, "make script.sh executable")?;
    Ok(f)
}

/// Paths that break naive path handling: non-ASCII, spaces, nesting.
pub fn unicode_paths() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "файл.txt", "кириллица в содержимом\n")?;
    f.commit_file(1, "with space.txt", "spaces in the name\n")?;
    f.commit_file(2, "каталог/вложенный.txt", "nested and non-ascii\n")?;
    Ok(f)
}

/// `n` stashes on top of a small history, working tree left clean.
pub fn with_stashes(n: i64) -> Result<Fixture> {
    let f = linear(2)?;
    for i in 0..n {
        f.write_file("work-in-progress.txt", &format!("unfinished work {i}\n"))?;
        // `-u` is required: the file is untracked, and a plain stash would skip it
        // and then fail with "no local changes to save".
        f.git_at(10 + i, &["stash", "push", "-u", "-m", &format!("wip {i}")])?;
    }
    Ok(f)
}

/// A long linear history, built for benchmarking rather than for reading.
///
/// Every commit touches the same single file, so the tree stays tiny and checking out
/// 10 000 commits costs nothing.
pub fn stress(n: i64) -> Result<Fixture> {
    let f = Fixture::init()?;
    // A loop of `git commit` would launch two processes per commit; at ten thousand
    // commits that is minutes on Windows. fast-import does the whole history in one.
    run_git_stdin(
        f.path(),
        &["fast-import", "--quiet"],
        &fast_import_stream(n),
    )?;
    // fast-import writes objects and refs but leaves the working tree empty.
    f.git(&["reset", "--hard", "main"])?;
    Ok(f)
}

/// Builds a fast-import stream for `n` linear commits.
fn fast_import_stream(n: i64) -> String {
    let mut stream = String::new();
    for i in 0..n {
        let message = format!("commit {i}");
        let content = format!("revision {i}\n");
        let stamp = BASE_TIMESTAMP + i * STEP_SECONDS;

        stream.push_str("commit refs/heads/main\n");
        stream.push_str(&format!("mark :{}\n", i + 1));
        stream.push_str(&format!(
            "author {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000\n"
        ));
        stream.push_str(&format!(
            "committer {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000\n"
        ));
        stream.push_str(&format!("data {}\n{message}\n", message.len()));
        // `from` must follow the commit message, per the fast-import grammar.
        if i > 0 {
            stream.push_str(&format!("from :{i}\n"));
        }
        stream.push_str("M 100644 inline data.txt\n");
        stream.push_str(&format!("data {}\n{content}\n", content.len()));
    }
    stream
}

/// Runs `git` with something fed to its standard input.
fn run_git_stdin(cwd: &Path, args: &[&str], input: &str) -> Result<String> {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = git_command(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
        // Dropping the handle closes the pipe, which is how fast-import learns the
        // stream has ended; without it the child waits forever.
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(FixtureError::Git {
            args: args.iter().map(|s| (*s).to_owned()).collect(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A parent repository with an initialised submodule at `vendor/lib`.
pub fn with_submodule() -> Result<Fixture> {
    let source = linear(2)?;
    let source_url = source.url_path();

    let mut f = Fixture::init()?;
    f.commit_file(0, "README.md", "parent repository\n")?;

    // Git 2.38 and later refuse submodules over `file://` by default, as a fix for
    // CVE-2022-39253. The allowance is scoped to this single command rather than
    // written into the repository config, so the fixture stays representative.
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--",
        &source_url,
        "vendor/lib",
    ])?;
    f.commit_staged(1, "add vendor/lib submodule")?;

    // The submodule was cloned from this directory and still refers to it.
    f._aux.push(source.into_temp_dir());
    Ok(f)
}

/// A repository with one linked worktree on its own branch.
pub fn with_worktree() -> Result<Fixture> {
    let mut f = linear(3)?;
    let aux = TempDir::new()?;
    // `worktree add` insists the target does not exist yet, so point at a child of
    // the temporary directory rather than the directory itself.
    let target = aux
        .path()
        .join("linked")
        .to_string_lossy()
        .replace('\\', "/");
    // Git forbids two checkouts of the same branch, so the worktree gets its own.
    f.git(&["worktree", "add", "-b", "feature-wt", &target])?;
    f._aux.push(aux);
    Ok(f)
}

/// A repository with `n` commits in a single line.
pub fn linear(n: i64) -> Result<Fixture> {
    let fixture = Fixture::init()?;
    for i in 0..n {
        fixture.commit_file(i, &format!("file{i}.txt"), &format!("content {i}\n"))?;
    }
    Ok(fixture)
}

/// `main` with a `dev` branch that diverged and was never merged.
pub fn branched() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "base.txt", "base\n")?;
    f.commit_file(1, "main-1.txt", "main one\n")?;

    f.git(&["switch", "-c", "dev", "HEAD~1"])?;
    f.commit_file(2, "dev-1.txt", "dev one\n")?;
    f.commit_file(3, "dev-2.txt", "dev two\n")?;

    f.git(&["switch", "main"])?;
    Ok(f)
}

/// The classic diamond: history splits in two and merges back.
pub fn diamond() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "base.txt", "base\n")?;

    f.git(&["switch", "-c", "dev"])?;
    f.commit_file(1, "dev.txt", "from dev\n")?;

    f.git(&["switch", "main"])?;
    f.commit_file(2, "main.txt", "from main\n")?;

    f.merge(3, &["dev"], "merge dev into main")?;
    Ok(f)
}

/// A single merge commit joining `main` and two side branches — three parents.
pub fn octopus() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "base.txt", "base\n")?;

    for (i, branch) in ["feature-a", "feature-b"].iter().enumerate() {
        f.git(&["switch", "-c", branch, "main"])?;
        f.commit_file(1 + i as i64, &format!("{branch}.txt"), "side\n")?;
    }

    f.git(&["switch", "main"])?;
    // `main` must carry a commit of its own first. While it is still an ancestor of both
    // branches, git fast-forwards HEAD to the first head and merges only the rest,
    // producing an ordinary two-parent merge instead of an octopus.
    f.commit_file(3, "main-own.txt", "main moved on\n")?;

    f.merge(4, &["feature-a", "feature-b"], "octopus merge")?;
    Ok(f)
}

/// Two histories with no common ancestor, as produced by an orphan branch.
pub fn two_roots() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "main.txt", "main history\n")?;
    f.commit_file(1, "main-2.txt", "more main\n")?;

    f.git(&["switch", "--orphan", "orphan"])?;
    // An orphan branch keeps the old index; clearing it makes the new root genuinely
    // independent instead of a copy of main with no parent.
    f.git(&["rm", "-rf", "--cached", "--ignore-unmatch", "."])?;
    f.commit_file(2, "orphan.txt", "unrelated history\n")?;

    f.git(&["switch", "main"])?;
    Ok(f)
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
    fn the_ambient_git_environment_is_cleared() {
        // Git exports GIT_DIR, GIT_INDEX_FILE and friends to its hooks. Inherited by a
        // fixture, they redirect every command at the *caller's* repository: running the
        // suite from a pre-commit hook would have committed into the real repository
        // instead of a temporary one. Clearing them is a safety property, not tidiness.
        let dir = TempDir::new().unwrap();
        let cmd = git_command(dir.path());
        let cleared: Vec<&str> = cmd
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .filter_map(|(key, _)| key.to_str())
            .collect();

        for name in AMBIENT_GIT_VARS {
            assert!(
                cleared.contains(name),
                "{name} must be cleared, cleared: {cleared:?}"
            );
        }
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
