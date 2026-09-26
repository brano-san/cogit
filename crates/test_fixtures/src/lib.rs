use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

pub const AUTHOR_NAME: &str = "Cogit Fixture";
pub const AUTHOR_EMAIL: &str = "fixture@cogit.test";
pub const BASE_TIMESTAMP: i64 = 1_767_225_600;
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

#[derive(Debug)]
pub struct Fixture {
    dir: TempDir,
    _aux: Vec<TempDir>,
}

impl Fixture {
    pub fn init() -> Result<Self> {
        let dir = TempDir::new()?;
        let fixture = Self {
            dir,
            _aux: Vec::new(),
        };
        // Pinned: the config below replaces git's, and with it any `[extensions]` a
        // reftable or sha256 default would have needed.
        fixture.git(&[
            "init",
            "--initial-branch=main",
            "--ref-format=files",
            "--object-format=sha1",
        ])?;

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

    #[must_use]
    pub fn git_dir(&self) -> PathBuf {
        let nested = self.dir.path().join(".git");
        if nested.is_dir() {
            nested
        } else {
            self.dir.path().to_path_buf()
        }
    }

    pub fn git(&self, args: &[&str]) -> Result<String> {
        self.run(args, None)
    }

    /// Runs a command inside a nested repository, such as a submodule checkout.
    pub fn git_in(&self, cwd: &Path, args: &[&str]) -> Result<String> {
        run_git(cwd, args, None)
    }

    pub fn git_at(&self, index: i64, args: &[&str]) -> Result<String> {
        self.run(args, Some(index))
    }

    fn run(&self, args: &[&str], date_index: Option<i64>) -> Result<String> {
        run_git(self.dir.path(), args, date_index)
    }

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

    pub fn commit_staged(&self, index: i64, message: &str) -> Result<String> {
        self.git_at(index, &["commit", "-m", message])?;
        self.oid("HEAD")
    }

    pub fn write_file(&self, name: &str, contents: &str) -> Result<()> {
        let target = self.dir.path().join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        Ok(())
    }

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

    pub fn oid(&self, rev: &str) -> Result<String> {
        Ok(self.git(&["rev-parse", rev])?.trim().to_owned())
    }

    fn into_temp_dir(self) -> TempDir {
        self.dir
    }

    fn url_path(&self) -> String {
        self.dir.path().to_string_lossy().replace('\\', "/")
    }
}

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
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_EDITOR",
    "GIT_SEQUENCE_EDITOR",
    "GIT_DEFAULT_REF_FORMAT",
    "GIT_DEFAULT_HASH",
];

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

pub fn empty() -> Result<Fixture> {
    Fixture::init()
}

pub fn bare() -> Result<Fixture> {
    let source = linear(3)?;
    let dir = TempDir::new()?;
    let src = source.path().to_string_lossy().into_owned();
    run_git(dir.path(), &["clone", "--bare", "--", &src, "."], None)?;
    Ok(Fixture {
        dir,
        _aux: Vec::new(),
    })
}

pub fn detached_head() -> Result<Fixture> {
    let f = linear(3)?;
    f.git(&["switch", "--detach", "HEAD~1"])?;
    Ok(f)
}

pub fn conflicted() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "conflict.txt", "original line\n")?;

    f.git(&["switch", "-c", "dev"])?;
    f.commit_file(1, "conflict.txt", "changed by dev\n")?;

    f.git(&["switch", "main"])?;
    f.commit_file(2, "conflict.txt", "changed by main\n")?;

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

pub fn renames() -> Result<Fixture> {
    let f = Fixture::init()?;
    let body = (0..20).map(|i| format!("line {i}\n")).collect::<String>();
    f.commit_file(0, "old-name.txt", &body)?;
    f.git(&["mv", "old-name.txt", "new-name.txt"])?;
    f.commit_staged(1, "rename old-name.txt to new-name.txt")?;
    Ok(f)
}

pub fn filemode_change() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "script.sh", "#!/bin/sh\necho hello\n")?;
    f.git(&["update-index", "--chmod=+x", "script.sh"])?;
    f.commit_staged(1, "make script.sh executable")?;
    Ok(f)
}

pub fn unicode_paths() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "файл.txt", "кириллица в содержимом\n")?;
    f.commit_file(1, "with space.txt", "spaces in the name\n")?;
    f.commit_file(2, "каталог/вложенный.txt", "nested and non-ascii\n")?;
    Ok(f)
}

/// A clone with an `origin` that has moved on: two commits ahead locally, one behind.
pub fn with_remote() -> Result<Fixture> {
    let mut f = linear(2)?;
    // Beside the repository, not in it: inside, they were untracked files of its own.
    let aux = TempDir::new()?;
    let remote = aux.path().join("origin.git");
    run_git(
        f.path(),
        &[
            "init",
            "--bare",
            "--initial-branch=main",
            &remote.to_string_lossy(),
        ],
        None,
    )?;
    f.git(&["remote", "add", "origin", &remote.to_string_lossy()])?;
    f.git(&["push", "--set-upstream", "origin", "main"])?;

    // The remote moves on through a second clone, so `origin/main` is genuinely ahead.
    let other = aux.path().join("other");
    run_git(
        aux.path(),
        &["clone", &remote.to_string_lossy(), &other.to_string_lossy()],
        None,
    )?;
    std::fs::write(
        other.join("from-remote.txt"),
        "remote work
",
    )?;
    run_git(&other, &["add", "--", "from-remote.txt"], None)?;
    run_git(&other, &["commit", "-m", "remote commit"], Some(30))?;
    run_git(&other, &["push"], None)?;

    f.commit_file(
        20,
        "local-a.txt",
        "local a
",
    )?;
    f.commit_file(
        21,
        "local-b.txt",
        "local b
",
    )?;
    f.git(&["fetch", "origin"])?;
    f._aux.push(aux);
    Ok(f)
}

pub fn with_stashes(n: i64) -> Result<Fixture> {
    let f = linear(2)?;
    for i in 0..n {
        f.write_file("work-in-progress.txt", &format!("unfinished work {i}\n"))?;
        f.git_at(10 + i, &["stash", "push", "-u", "-m", &format!("wip {i}")])?;
    }
    Ok(f)
}

pub fn stress(n: i64) -> Result<Fixture> {
    let f = Fixture::init()?;
    run_git_stdin(
        f.path(),
        &["fast-import", "--quiet"],
        &fast_import_stream(n),
    )?;
    f.git(&["reset", "--hard", "main"])?;
    Ok(f)
}

/// A broad tree rather than a long history: `n` files, then one commit touching one.
/// Rename and copy detection scale with the tree, not with the number of commits.
pub fn wide(n: i64) -> Result<Fixture> {
    let f = Fixture::init()?;
    run_git_stdin(
        f.path(),
        &["fast-import", "--quiet"],
        &wide_import_stream(n),
    )?;
    f.git(&["reset", "--hard", "main"])?;
    Ok(f)
}

fn wide_import_stream(n: i64) -> String {
    let mut stream = String::new();
    for (mark, stamp) in [(1, BASE_TIMESTAMP), (2, BASE_TIMESTAMP + STEP_SECONDS)] {
        let message = if mark == 1 {
            "seed tree"
        } else {
            "touch one file"
        };
        stream.push_str(
            "commit refs/heads/main
",
        );
        stream.push_str(&format!(
            "mark :{mark}
"
        ));
        stream.push_str(&format!(
            "author {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000
"
        ));
        stream.push_str(&format!(
            "committer {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000
"
        ));
        stream.push_str(&format!(
            "data {}
{message}
",
            message.len()
        ));
        if mark == 2 {
            stream.push_str(
                "from :1
",
            );
            let content = "edited
";
            stream.push_str(
                "M 100644 inline dir000/file0000.txt
",
            );
            stream.push_str(&format!(
                "data {}
{content}",
                content.len()
            ));
            continue;
        }
        for i in 0..n {
            let content = format!(
                "file {i}
"
            );
            stream.push_str(&format!(
                "M 100644 inline dir{:03}/file{i:04}.txt
",
                i % 16
            ));
            stream.push_str(&format!(
                "data {}
{content}",
                content.len()
            ));
        }
    }
    stream
}

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
        if i > 0 {
            stream.push_str(&format!("from :{i}\n"));
        }
        stream.push_str("M 100644 inline data.txt\n");
        stream.push_str(&format!("data {}\n{content}\n", content.len()));
    }
    stream
}

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

pub fn with_submodule() -> Result<Fixture> {
    let source = linear(2)?;
    let source_url = source.url_path();

    let mut f = Fixture::init()?;
    f.commit_file(0, "README.md", "parent repository\n")?;

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
    allow_local_submodules(&f)?;

    f._aux.push(source.into_temp_dir());
    Ok(f)
}

pub fn with_worktree() -> Result<Fixture> {
    let mut f = linear(3)?;
    let aux = TempDir::new()?;
    let target = aux
        .path()
        .join("linked")
        .to_string_lossy()
        .replace('\\', "/");
    f.git(&["worktree", "add", "-b", "feature-wt", &target])?;
    f._aux.push(aux);
    Ok(f)
}

/// Built with `fast-import`: the previous loop spawned two `git` processes per commit, and
/// three hundred tests pay for this fixture (R-56). The shape is pinned by `tests/linear.rs`.
pub fn linear(n: i64) -> Result<Fixture> {
    let fixture = Fixture::init()?;
    if n <= 0 {
        return Ok(fixture);
    }

    run_git_stdin(
        fixture.path(),
        &["fast-import", "--quiet"],
        &linear_import_stream(n),
    )?;
    fixture.git(&["reset", "--hard", "main"])?;
    Ok(fixture)
}

fn linear_import_stream(n: i64) -> String {
    let mut stream = String::new();
    for i in 0..n {
        let message = format!("commit {i}");
        let content = format!("content {i}\n");
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
        if i > 0 {
            stream.push_str(&format!("from :{i}\n"));
        }
        stream.push_str(&format!("M 100644 inline file{i}.txt\n"));
        stream.push_str(&format!("data {}\n{content}", content.len()));
    }
    stream
}

/// One commit in a `fast-import` stream. Building a shape this way costs one process
/// instead of two per commit, and the shapes are used in hundreds of tests (R-56).
struct ImportCommit<'a> {
    mark: i64,
    index: i64,
    message: &'a str,
    /// `None` starts a new root.
    from: Option<i64>,
    merge: Option<i64>,
    files: &'a [(&'a str, &'a str)],
}

fn import_commit(stream: &mut String, branch: &str, commit: &ImportCommit<'_>) {
    let stamp = BASE_TIMESTAMP + commit.index * STEP_SECONDS;
    stream.push_str(&format!("commit refs/heads/{branch}\n"));
    stream.push_str(&format!("mark :{}\n", commit.mark));
    stream.push_str(&format!(
        "author {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000\n"
    ));
    stream.push_str(&format!(
        "committer {AUTHOR_NAME} <{AUTHOR_EMAIL}> {stamp} +0000\n"
    ));
    stream.push_str(&format!(
        "data {}\n{}\n",
        commit.message.len(),
        commit.message
    ));
    if let Some(from) = commit.from {
        stream.push_str(&format!("from :{from}\n"));
    }
    if let Some(merge) = commit.merge {
        stream.push_str(&format!("merge :{merge}\n"));
    }
    for (name, contents) in commit.files {
        stream.push_str(&format!("M 100644 inline {name}\n"));
        stream.push_str(&format!("data {}\n{contents}", contents.len()));
    }
}

pub fn branched() -> Result<Fixture> {
    let f = Fixture::init()?;
    let mut stream = String::new();

    let base = ImportCommit {
        mark: 1,
        index: 0,
        message: "commit 0",
        from: None,
        merge: None,
        files: &[("base.txt", "base\n")],
    };
    import_commit(&mut stream, "main", &base);
    import_commit(
        &mut stream,
        "main",
        &ImportCommit {
            mark: 2,
            index: 1,
            message: "commit 1",
            from: Some(1),
            merge: None,
            files: &[("main-1.txt", "main one\n")],
        },
    );
    // dev leaves main one commit back, from the base.
    import_commit(
        &mut stream,
        "dev",
        &ImportCommit {
            mark: 3,
            index: 2,
            message: "commit 2",
            from: Some(1),
            merge: None,
            files: &[("dev-1.txt", "dev one\n")],
        },
    );
    import_commit(
        &mut stream,
        "dev",
        &ImportCommit {
            mark: 4,
            index: 3,
            message: "commit 3",
            from: Some(3),
            merge: None,
            files: &[("dev-2.txt", "dev two\n")],
        },
    );

    run_git_stdin(f.path(), &["fast-import", "--quiet"], &stream)?;
    f.git(&["reset", "--hard", "main"])?;
    Ok(f)
}

pub fn diamond() -> Result<Fixture> {
    let f = Fixture::init()?;
    let mut stream = String::new();

    import_commit(
        &mut stream,
        "main",
        &ImportCommit {
            mark: 1,
            index: 0,
            message: "commit 0",
            from: None,
            merge: None,
            files: &[("base.txt", "base\n")],
        },
    );
    import_commit(
        &mut stream,
        "dev",
        &ImportCommit {
            mark: 2,
            index: 1,
            message: "commit 1",
            from: Some(1),
            merge: None,
            files: &[("dev.txt", "from dev\n")],
        },
    );
    import_commit(
        &mut stream,
        "main",
        &ImportCommit {
            mark: 3,
            index: 2,
            message: "commit 2",
            from: Some(1),
            merge: None,
            files: &[("main.txt", "from main\n")],
        },
    );
    // `from` is the first parent and `merge` the second: the lane algorithm reads the
    // first parent as the mainline.
    import_commit(
        &mut stream,
        "main",
        &ImportCommit {
            mark: 4,
            index: 3,
            message: "merge dev into main",
            from: Some(3),
            merge: Some(2),
            // fast-import does not merge trees; the result has to be stated. Here that is
            // simply both sides, because the two branches touch different files.
            files: &[(
                "dev.txt",
                "from dev
",
            )],
        },
    );

    run_git_stdin(f.path(), &["fast-import", "--quiet"], &stream)?;
    f.git(&["reset", "--hard", "main"])?;
    Ok(f)
}

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

pub fn two_roots() -> Result<Fixture> {
    let f = Fixture::init()?;
    f.commit_file(0, "main.txt", "main history\n")?;
    f.commit_file(1, "main-2.txt", "more main\n")?;

    f.git(&["switch", "--orphan", "orphan"])?;
    f.git(&["rm", "-rf", "--cached", "--ignore-unmatch", "."])?;
    f.commit_file(2, "orphan.txt", "unrelated history\n")?;

    f.git(&["switch", "main"])?;
    Ok(f)
}

/// A submodule that itself has one. The second level is what a naive `.gitmodules` reader
/// misses, since only the top file is in the parent's tree (M3).
pub fn with_nested_submodule() -> Result<Fixture> {
    let inner = linear(2)?;
    let inner_url = inner.url_path();

    let middle = Fixture::init()?;
    middle.commit_file(0, "middle.md", "the middle repository\n")?;
    middle.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--",
        &inner_url,
        "deep/inner",
    ])?;
    middle.commit_staged(1, "add deep/inner submodule")?;
    let middle_url = middle.url_path();

    let mut f = Fixture::init()?;
    f.commit_file(2, "README.md", "parent repository\n")?;
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--",
        &middle_url,
        "vendor/middle",
    ])?;
    f.commit_staged(3, "add vendor/middle submodule")?;
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "update",
        "--init",
        "--recursive",
    ])?;

    allow_local_submodules(&f)?;

    f._aux.push(middle.into_temp_dir());
    f._aux.push(inner.into_temp_dir());
    Ok(f)
}

/// The submodules here are on local paths, which git clones only where the user allowed
/// it (CVE-2022-39253). This is that permission, in the repository's own config.
fn allow_local_submodules(f: &Fixture) -> Result<()> {
    f.git(&["config", "protocol.file.allow", "always"])
        .map(drop)
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
