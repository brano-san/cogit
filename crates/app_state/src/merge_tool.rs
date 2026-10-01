use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LAUNCH_ROOT: &str = "cogit-merge";
const OWNER_MARKER: &str = ".owner";
pub const DEFAULT_ARGS: &str = "{base} {ours} {theirs} {result}";
const POLL: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown placeholder `{0}` in the merge tool arguments")]
    UnknownPlaceholder(String),
    #[error("unbalanced quote in the merge tool command")]
    UnbalancedQuote,
    #[error("the merge tool program is empty")]
    EmptyProgram,
    #[error("no merge tool is configured")]
    NoToolConfigured,
    #[error("the merge tool command uses shell syntax that is not supported: {0}")]
    ShellCommandUnsupported(String),
    #[error("merge tool `{0}` is not supported; configure a custom command")]
    UnknownTool(String),
    #[error("a merge tool is already running for this file")]
    AlreadyRunning,
    #[error("merge tool I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("cannot start `{program}`: {source}")]
    SpawnFailed { program: String, source: io::Error },
}

#[derive(Debug, Clone)]
pub struct ToolFiles {
    pub dir: PathBuf,
    pub base: PathBuf,
    pub ours: PathBuf,
    pub theirs: PathBuf,
    pub result: PathBuf,
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect()
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn prepare(
    temp_root: &Path,
    repo_root: &Path,
    path: &str,
    base: Option<&[u8]>,
    ours: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> io::Result<ToolFiles> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let id = format!(
        "{}-{nanos:x}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = temp_root.join(LAUNCH_ROOT).join(id);
    std::fs::create_dir_all(&dir)?;
    let built = (|| {
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let (stem, ext) = match file_name.rfind('.') {
            Some(i) if i > 0 => file_name.split_at(i),
            _ => (file_name, ""),
        };
        let (stem, ext) = (sanitize(stem), sanitize(ext));
        let side = |tag: &str, data: Option<&[u8]>| -> io::Result<PathBuf> {
            let p = dir.join(format!("{stem}.{tag}{ext}"));
            std::fs::write(&p, data.unwrap_or_default())?;
            Ok(p)
        };
        let files = ToolFiles {
            base: side("base", base)?,
            ours: side("ours", ours)?,
            theirs: side("theirs", theirs)?,
            result: repo_root.join(path),
            dir: dir.clone(),
        };
        std::fs::write(
            dir.join(OWNER_MARKER),
            format!("{} {}", std::process::id(), unix_secs(SystemTime::now())),
        )?;
        Ok(files)
    })();
    if built.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    built
}

impl ToolFiles {
    pub fn cleanup(&self) {
        match std::fs::remove_dir_all(&self.dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = ?e, dir = ?self.dir, "merge tool temp cleanup failed"),
        }
    }
}

pub fn sweep(temp_root: &Path, max_age: Duration, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(temp_root.join(LAUNCH_ROOT)) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let dir = entry.path();
        let stamped = std::fs::read_to_string(dir.join(OWNER_MARKER))
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map(|s| UNIX_EPOCH + Duration::from_secs(s));
        let Some(stamp) = stamped.or_else(|| entry.metadata().ok()?.modified().ok()) else {
            continue;
        };
        if now.duration_since(stamp).is_ok_and(|age| age > max_age)
            && std::fs::remove_dir_all(&dir).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

/// Pops the first argument off `line`, returning it and the remaining text.
///
/// Whitespace separates; `"..."` groups (a backslash is literal there except in `\"`, so a
/// quoted token cannot end in a backslash; put paths in placeholders); `'...'` is literal;
/// adjacent pieces concatenate; `""` is an empty argument.
fn next_token(line: &str) -> Result<Option<(String, &str)>, ToolError> {
    let line = line.trim_start();
    if line.is_empty() {
        return Ok(None);
    }
    let mut out = String::new();
    let mut chars = line.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        match c {
            c if c.is_whitespace() => return Ok(Some((out, &line[i..]))),
            '"' => {
                chars.next();
                loop {
                    match chars.next() {
                        None => return Err(ToolError::UnbalancedQuote),
                        Some((_, '"')) => break,
                        Some((_, '\\')) if matches!(chars.peek(), Some(&(_, '"'))) => {
                            chars.next();
                            out.push('"');
                        }
                        Some((_, ch)) => out.push(ch),
                    }
                }
            }
            '\'' => {
                chars.next();
                loop {
                    match chars.next() {
                        None => return Err(ToolError::UnbalancedQuote),
                        Some((_, '\'')) => break,
                        Some((_, ch)) => out.push(ch),
                    }
                }
            }
            _ => {
                out.push(c);
                chars.next();
            }
        }
    }
    Ok(Some((out, "")))
}

pub fn tokenize(line: &str) -> Result<Vec<String>, ToolError> {
    let mut rest = line;
    let mut out = Vec::new();
    while let Some((tok, r)) = next_token(rest)? {
        out.push(tok);
        rest = r;
    }
    Ok(out)
}

/// Replaces `{base}`, `{ours}`, `{theirs}`, `{result}`; `{{` and `}}` are literal braces.
pub fn substitute(token: &str, files: &ToolFiles) -> Result<String, ToolError> {
    let mut out = String::new();
    let mut chars = token.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => return Err(ToolError::UnknownPlaceholder(name)),
                    }
                }
                let p = match name.as_str() {
                    "base" => &files.base,
                    "ours" => &files.ours,
                    "theirs" => &files.theirs,
                    "result" => &files.result,
                    _ => return Err(ToolError::UnknownPlaceholder(name)),
                };
                out.push_str(&p.to_string_lossy());
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn build_command(
    program: &str,
    args_template: &str,
    files: &ToolFiles,
) -> Result<ToolCommand, ToolError> {
    let program = program.trim();
    let program = program
        .strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .unwrap_or(program)
        .trim();
    if program.is_empty() {
        return Err(ToolError::EmptyProgram);
    }
    let template = if args_template.trim().is_empty() {
        DEFAULT_ARGS
    } else {
        args_template
    };
    let args = tokenize(template)?
        .iter()
        .map(|t| substitute(t, files))
        .collect::<Result<_, _>>()?;
    Ok(ToolCommand {
        program: program.to_owned(),
        args,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitToolConfig {
    pub tool: Option<String>,
    pub cmd: Option<String>,
    pub path: Option<String>,
}

fn builtin(tool: &str) -> Option<(&'static str, &'static str)> {
    Some(match tool {
        "meld" => ("meld", "--output {result} {ours} {base} {theirs}"),
        "kdiff3" => ("kdiff3", "{base} {ours} {theirs} -o {result}"),
        "vscode" => ("code", "--wait --merge {ours} {theirs} {base} {result}"),
        "bc" | "bc3" => ("bcomp", "{ours} {theirs} {base} {result}"),
        "p4merge" => ("p4merge", "{base} {ours} {theirs} {result}"),
        "winmerge" => (
            "WinMergeU",
            "/e /u /wl /wr /dl Ours /dr Theirs /o {result} {ours} {theirs}",
        ),
        _ => return None,
    })
}

fn nonblank(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Converts a git `mergetool.<tool>.cmd` (no shell is run) or a built-in tool name.
pub fn from_git_config(cfg: &GitToolConfig) -> Result<(String, String), ToolError> {
    if let Some(cmd) = nonblank(&cfg.cmd) {
        if ["&&", "||", ";", "|", ">", "$("]
            .iter()
            .any(|s| cmd.contains(s))
        {
            return Err(ToolError::ShellCommandUnsupported(cmd.to_owned()));
        }
        let mut converted = cmd.to_owned();
        for (var, ph) in [
            ("BASE", "{base}"),
            ("LOCAL", "{ours}"),
            ("REMOTE", "{theirs}"),
            ("MERGED", "{result}"),
        ] {
            converted = converted
                .replace(&format!("${{{var}}}"), ph)
                .replace(&format!("${var}"), ph);
        }
        let (program, rest) = next_token(&converted)?.ok_or(ToolError::EmptyProgram)?;
        return Ok((program, rest.trim().to_owned()));
    }
    let tool = nonblank(&cfg.tool).ok_or(ToolError::NoToolConfigured)?;
    let (program, args) = builtin(tool).ok_or_else(|| ToolError::UnknownTool(tool.to_owned()))?;
    let program = nonblank(&cfg.path).unwrap_or(program);
    Ok((program.to_owned(), args.to_owned()))
}

pub fn has_conflict_markers(text: &str) -> bool {
    let marker = |line: &str, m: &str| {
        line.strip_prefix(m)
            .is_some_and(|r| r.is_empty() || r.starts_with(' '))
    };
    let (mut open, mut close) = (false, false);
    for line in text.lines() {
        open |= marker(line, "<<<<<<<");
        close |= marker(line, ">>>>>>>");
    }
    open && close
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ToolKey {
    pub repo: u32,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExit {
    pub code: Option<i32>,
    pub canceled: bool,
    pub files_dir_removed: bool,
}

#[derive(Debug)]
struct Running {
    child: Arc<Mutex<Child>>,
    canceled: Arc<AtomicBool>,
}

#[derive(Clone, Default, Debug)]
pub struct MergeTools {
    running: Arc<Mutex<HashMap<ToolKey, Running>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn spawn(command: &ToolCommand, cwd: &Path) -> io::Result<Child> {
    let mut cmd = Command::new(&command.program);
    cmd.args(&command.args)
        .current_dir(cwd)
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
}

impl MergeTools {
    pub fn start(
        &self,
        key: ToolKey,
        command: ToolCommand,
        cwd: &Path,
        files: ToolFiles,
        on_exit: impl FnOnce(ToolExit) + Send + 'static,
    ) -> Result<(), ToolError> {
        let mut registry = lock(&self.running);
        if registry.contains_key(&key) {
            files.cleanup();
            return Err(ToolError::AlreadyRunning);
        }
        let child = match spawn(&command, cwd) {
            Ok(c) => Arc::new(Mutex::new(c)),
            Err(source) => {
                files.cleanup();
                return Err(ToolError::SpawnFailed {
                    program: command.program,
                    source,
                });
            }
        };
        let canceled = Arc::new(AtomicBool::new(false));
        registry.insert(
            key.clone(),
            Running {
                child: child.clone(),
                canceled: canceled.clone(),
            },
        );
        let this = self.clone();
        let waiter_files = files.clone();
        let waiter_key = key.clone();
        let spawned = std::thread::Builder::new()
            .name("merge-tool-wait".into())
            .spawn(move || {
                // ponytail: polls try_wait so cancel can kill; swap for a blocking wait if latency matters
                let code = loop {
                    match lock(&child).try_wait() {
                        Ok(Some(status)) => break status.code(),
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(error = ?e, "waiting for the merge tool failed");
                            break None;
                        }
                    }
                    std::thread::sleep(POLL);
                };
                lock(&this.running).remove(&waiter_key);
                waiter_files.cleanup();
                on_exit(ToolExit {
                    code,
                    canceled: canceled.load(Ordering::SeqCst),
                    files_dir_removed: !waiter_files.dir.exists(),
                });
            });
        if let Err(e) = spawned {
            if let Some(r) = registry.remove(&key) {
                let _ = lock(&r.child).kill();
            }
            files.cleanup();
            return Err(ToolError::Io(e));
        }
        Ok(())
    }

    pub fn cancel(&self, key: &ToolKey) -> bool {
        let registry = lock(&self.running);
        let Some(r) = registry.get(key) else {
            return false;
        };
        r.canceled.store(true, Ordering::SeqCst);
        if let Err(e) = lock(&r.child).kill() {
            tracing::warn!(error = ?e, "killing the merge tool failed");
        }
        true
    }

    pub fn is_running(&self, key: &ToolKey) -> bool {
        lock(&self.running).contains_key(key)
    }

    pub fn running(&self) -> Vec<ToolKey> {
        lock(&self.running).keys().cloned().collect()
    }
}
