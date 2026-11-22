//! User commands (F-954): words split, then placeholders filled; no shell, no environment.

use crate::desktop::{Platform, native_path};
use crate::location::RepoLocation;
use crate::merge_tool::tokenize;
use crate::{AppState, RepoId};
use git_engine::{GitError, NetworkStop, UserCommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolContext {
    /// Relative to the root, `/`-separated.
    pub file: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub files: Vec<String>,
    #[serde(default)]
    #[specta(optional)]
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequest {
    pub name: String,
    pub program: String,
    pub args: String,
    pub cwd: String,
    /// Started and left to run, output unseen (a GUI tool); else captured into the journal.
    pub detached: bool,
    pub context: ToolContext,
    #[serde(default)]
    #[specta(optional)]
    pub confirmation: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub confirm: bool,
    #[serde(default)]
    #[specta(optional)]
    pub source: Option<RepositoryTool>,
    #[serde(default)]
    #[specta(optional)]
    pub output: ToolOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryTool {
    pub hash: String,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolPreview {
    pub line: String,
    pub cwd: String,
    pub confirmation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub program: String,
    pub args: String,
    pub cwd: String,
    pub places: Vec<String>,
    pub detached: bool,
    pub confirm: bool,
    pub refresh: bool,
    pub patterns: String,
    pub shortcut: String,
    pub output: ToolOutput,
    pub replace_message: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ToolOutput {
    #[default]
    Panel,
    Notification,
    Message,
    Silent,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolFile {
    pub hash: String,
    pub commands: Vec<ToolDefinition>,
    pub trusted: bool,
}

const COMMANDS_FILE: &str = ".cogit/commands.json";
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_COMMANDS: usize = 50;

fn invalid(message: impl Into<String>) -> GitError {
    GitError::InvalidState(message.into())
}

fn io_error(err: std::io::Error) -> GitError {
    GitError::Io(err.to_string())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn load_bytes(path: &Path) -> Result<Option<Vec<u8>>, GitError> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_error(err)),
    };
    if !metadata.is_file() {
        return Err(invalid(format!(
            "{} must be a regular file",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(io_error)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(invalid(format!("{} exceeds 256 KiB", path.display())));
    }
    Ok(Some(bytes))
}

fn parse_commands(path: &Path, bytes: &[u8]) -> Result<Vec<ToolDefinition>, GitError> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let commands: Vec<ToolDefinition> = serde_json::from_slice(bytes)
        .map_err(|err| invalid(format!("{}: {err}", path.display())))?;
    validate_commands(&commands)?;
    Ok(commands)
}

fn validate_commands(commands: &[ToolDefinition]) -> Result<(), GitError> {
    if commands.len() > MAX_COMMANDS {
        return Err(invalid("A command file can contain up to 50 commands."));
    }
    if commands
        .iter()
        .any(|command| command.name.trim().is_empty() || command.program.trim().is_empty())
    {
        return Err(invalid("Every command must have a name and a program."));
    }
    Ok(())
}

pub fn import_commands(path: &Path) -> Result<Vec<ToolDefinition>, GitError> {
    let bytes =
        load_bytes(path)?.ok_or_else(|| invalid(format!("{} does not exist", path.display())))?;
    parse_commands(path, &bytes)
}

pub fn export_commands(path: &Path, commands: &[ToolDefinition]) -> Result<(), GitError> {
    validate_commands(commands)?;
    let bytes = serde_json::to_vec_pretty(commands).map_err(|err| invalid(err.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(invalid("The exported commands exceed 256 KiB."));
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".writing");
    let temporary = PathBuf::from(temporary);
    let mut file = std::fs::File::create_new(&temporary).map_err(io_error)?;
    let written = file.write_all(&bytes);
    drop(file);
    let result = written.and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err()
        && let Err(err) = std::fs::remove_file(&temporary)
    {
        tracing::warn!(error = ?err, "cannot remove an incomplete command export");
    }
    result.map_err(io_error)
}

fn trust_key(root: &Path) -> String {
    format!(
        "repositoryToolTrust:{}",
        digest(root.to_string_lossy().as_bytes())
    )
}

fn is_trusted(config_dir: &Path, root: &Path, hash: &str) -> Result<bool, GitError> {
    let document = crate::settings::try_read_document(config_dir).map_err(io_error)?;
    Ok(document
        .get(trust_key(root))
        .and_then(serde_json::Value::as_str)
        == Some(hash))
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolRun {
    pub record: u32,
    /// `None` when the user stopped it; a detached run that started is `0`.
    pub exit_code: Option<i32>,
    pub presentation: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ToolArgsError {
    #[error("the program is empty")]
    EmptyProgram,
    #[error("unbalanced quote in the arguments")]
    UnbalancedQuote,
    #[error("unknown placeholder `{0}`; see the placeholder list in Preferences ▸ Tools")]
    Unknown(String),
    #[error("`{0}` has nothing to stand for here")]
    Missing(String),
    #[error("`${{files}}` must be a separate argument")]
    FilesAsWord,
}

const SHORT_COMMIT: usize = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values {
    pub repo: String,
    pub file: Option<String>,
    pub relfile: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub files: Vec<String>,
    pub upstream: String,
    pub remote: String,
    pub issue: String,
    pub line: Option<u32>,
}

impl Values {
    #[must_use]
    pub fn of(platform: Platform, root: &Path, context: &ToolContext) -> Self {
        let root = root.to_string_lossy().replace('\\', "/");
        let absolute = |file: &str| {
            native_path(
                platform,
                &format!(
                    "{}/{}",
                    root.trim_end_matches('/'),
                    file.trim_start_matches('/')
                ),
            )
        };
        let relfile = context
            .file
            .as_deref()
            .map(|file| file.trim_start_matches('/'))
            .filter(|file| !file.is_empty());
        Self {
            repo: native_path(platform, &root),
            file: relfile.map(absolute),
            relfile: relfile.map(str::to_owned),
            commit: context.commit.clone().filter(|commit| !commit.is_empty()),
            branch: context.branch.clone().filter(|branch| !branch.is_empty()),
            files: if context.files.is_empty() {
                relfile.into_iter().map(absolute).collect()
            } else {
                context
                    .files
                    .iter()
                    .filter(|file| !file.is_empty())
                    .map(|file| absolute(file))
                    .collect()
            },
            line: context.line.filter(|line| *line > 0),
            ..Self::default()
        }
    }

    fn get(&self, name: &str) -> Result<Option<String>, ToolArgsError> {
        Ok(match name {
            "repo" | "root" => Some(self.repo.clone()),
            "file" => self.file.clone(),
            "relfile" => self.relfile.clone(),
            "commit" => self.commit.clone(),
            "shortcommit" => self
                .commit
                .as_ref()
                .map(|commit| commit.chars().take(SHORT_COMMIT).collect()),
            "branch" => Some(self.branch.clone().unwrap_or_default()),
            "upstream" => Some(self.upstream.clone()),
            "remote" => Some(self.remote.clone()),
            "issue" => Some(self.issue.clone()),
            "line" => self.line.map(|line| line.to_string()),
            "files" => return Err(ToolArgsError::FilesAsWord),
            _ => return Err(ToolArgsError::Unknown(format!("${{{name}}}"))),
        })
    }
}

/// `${name}` filled in inside one word, which stays one word; a lone `$` is itself.
pub fn expand_word(word: &str, values: &Values) -> Result<String, ToolArgsError> {
    let mut out = String::new();
    let mut rest = word;
    while let Some(at) = rest.find("${") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 2..];
        let Some(close) = tail.find('}') else {
            return Err(ToolArgsError::Unknown(rest[at..].to_owned()));
        };
        let name = &tail[..close];
        let value = values
            .get(name)?
            .ok_or_else(|| ToolArgsError::Missing(format!("${{{name}}}")))?;
        out.push_str(&value);
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Split first, then expanded word by word: a path with spaces stays one argument.
pub fn expand_args(args: &str, values: &Values) -> Result<Vec<String>, ToolArgsError> {
    let mut args_out = Vec::new();
    for word in tokenize(args).map_err(|_| ToolArgsError::UnbalancedQuote)? {
        if word == "${files}" {
            if values.files.is_empty() {
                return Err(ToolArgsError::Missing(word));
            }
            args_out.extend(values.files.iter().cloned());
        } else {
            args_out.push(expand_word(&word, values)?);
        }
    }
    Ok(args_out)
}

fn single(field: &str, values: &Values) -> Result<String, ToolArgsError> {
    let field = field.trim();
    let field = field
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(field)
        .trim();
    expand_word(field, values)
}

pub fn prepare(
    request: &ToolRequest,
    values: &Values,
) -> Result<(String, Vec<String>, PathBuf), ToolArgsError> {
    let program = single(&request.program, values)?;
    if program.is_empty() {
        return Err(ToolArgsError::EmptyProgram);
    }
    let args = expand_args(&request.args, values)?;
    let cwd = single(&request.cwd, values)?;
    let cwd = if cwd.is_empty() {
        PathBuf::from(&values.repo)
    } else {
        PathBuf::from(&values.repo).join(cwd)
    };
    Ok((program, args, cwd))
}

pub const SSH_REFUSAL: &str =
    "User commands run on this computer, and a repository over SSH has no path here.";

impl AppState {
    pub(crate) fn local_tool_metadata(
        &self,
        repo: RepoId,
        global_pattern: &str,
        issue: bool,
    ) -> Result<git_engine::ToolMetadata, GitError> {
        self.handle(repo)?.tool_metadata(global_pattern, issue)
    }

    pub(crate) fn local_repository_tool_file(
        &self,
        repo: RepoId,
    ) -> Result<Option<ToolFile>, GitError> {
        let path = self.root_of(repo)?.join(COMMANDS_FILE);
        let Some(bytes) = load_bytes(&path)? else {
            return Ok(None);
        };
        Ok(Some(ToolFile {
            hash: digest(&bytes),
            commands: parse_commands(&path, &bytes)?,
            trusted: false,
        }))
    }

    pub fn repository_tools(
        &self,
        repo: RepoId,
        config_dir: &Path,
    ) -> Result<Option<ToolFile>, GitError> {
        let root = self.root_of(repo)?;
        let Some(mut file) = self.repository_tool_file(repo)? else {
            return Ok(None);
        };
        file.trusted = !matches!(RepoLocation::of(&root), RepoLocation::Ssh { .. })
            && is_trusted(config_dir, &root, &file.hash)?;
        Ok(Some(file))
    }

    pub fn trust_repository_tools(
        &self,
        repo: RepoId,
        config_dir: &Path,
        hash: &str,
    ) -> Result<(), GitError> {
        let root = self.desktop_tool_root(repo)?;
        let file = self
            .repository_tool_file(repo)?
            .filter(|file| file.hash == hash)
            .ok_or_else(|| {
                invalid("Repository commands changed. Reopen the menu and review them again.")
            })?;
        crate::settings::write_key(
            config_dir,
            &trust_key(&root),
            serde_json::Value::String(file.hash),
        )
        .map_err(io_error)
    }

    fn desktop_tool_root(&self, repo: RepoId) -> Result<PathBuf, GitError> {
        let root = self.root_of(repo)?;
        if matches!(RepoLocation::of(&root), RepoLocation::Ssh { .. }) {
            return Err(invalid(SSH_REFUSAL));
        }
        Ok(root)
    }

    fn expanded_tool(
        &self,
        repo: RepoId,
        request: &ToolRequest,
        config_dir: &Path,
    ) -> Result<(PathBuf, ToolRequest, ExpandedTool), GitError> {
        let root = self.desktop_tool_root(repo)?;
        let mut request = request.clone();
        if let Some(source) = &request.source {
            let file = self
                .repository_tools(repo, config_dir)?
                .filter(|file| file.hash == source.hash && file.trusted)
                .ok_or_else(|| {
                    invalid("Trust the current .cogit/commands.json before running its commands.")
                })?;
            let command = file
                .commands
                .get(source.index as usize)
                .ok_or_else(|| invalid("The repository command no longer exists."))?;
            if request.detached != command.detached {
                return Err(invalid(
                    "The repository command changed. Reopen the menu and try again.",
                ));
            }
            request.name.clone_from(&command.name);
            request.program.clone_from(&command.program);
            request.args.clone_from(&command.args);
            request.cwd.clone_from(&command.cwd);
            request.confirm = command.confirm;
            request.output = command.output;
        }
        let fields = [&request.program, &request.args, &request.cwd];
        let has = |word: &str| fields.iter().any(|field| field.contains(word));
        let mut values = Values::of(Platform::current(), &root, &request.context);
        if ["${branch}", "${upstream}", "${remote}", "${issue}"]
            .iter()
            .any(|word| has(word))
        {
            let global = if has("${issue}") {
                crate::settings::try_read_document(config_dir).map_err(io_error)?["settings"]["commitPrefixPattern"]
                    .as_str().unwrap_or_default().to_owned()
            } else {
                String::new()
            };
            let metadata = self.tool_metadata(repo, &global, has("${issue}"))?;
            values.branch = metadata.branch;
            values.upstream = metadata.upstream;
            values.remote = metadata.remote;
            values.issue = metadata.issue;
        }
        let (program, args, cwd) = prepare(&request, &values)
            .map_err(|err| invalid(format!("{}: {err}", request.name)))?;
        Ok((root, request, ExpandedTool { program, args, cwd }))
    }

    pub fn preview_user_command(
        &self,
        repo: RepoId,
        request: &ToolRequest,
        config_dir: &Path,
    ) -> Result<ToolPreview, GitError> {
        let (root, request, expanded) = self.expanded_tool(repo, request, config_dir)?;
        Ok(ToolPreview {
            line: expanded.command(&request.name).line(),
            cwd: expanded.cwd.to_string_lossy().into_owned(),
            confirmation: expanded.confirmation(&root, &request)?,
        })
    }

    /// On this computer, not through the agent: WSL is reached by its `\\wsl.localhost\…` path.
    pub fn run_user_command(
        &self,
        repo: RepoId,
        request: &ToolRequest,
        stop: &NetworkStop,
        config_dir: &Path,
    ) -> Result<ToolRun, GitError> {
        let (root, request, expanded) = self.expanded_tool(repo, request, config_dir)?;
        if request.confirm
            && request.confirmation.as_deref() != Some(&expanded.confirmation(&root, &request)?)
        {
            return Err(invalid(
                "Confirm the expanded command before running it. Its arguments or working directory may have changed.",
            ));
        }
        let command = expanded.command(&request.name);
        let mut record = if request.detached {
            command.start()?;
            let mut record =
                git_engine::GitOutput::record(&root, command.line(), Some(0), "", "", 0);
            record.operation = format!("Tool: {}", request.name);
            record.summary = "Started; it runs on its own".to_owned();
            record
        } else {
            command.run(&root, stop)?
        };
        let mut presentation = None;
        if !request.detached && record.exit_code == Some(0) {
            presentation = match request.output {
                ToolOutput::Notification => Some(output_tail(&record.stdout, &record.stderr)),
                ToolOutput::Message => Some(record.stdout.clone()),
                ToolOutput::Panel | ToolOutput::Silent => None,
            };
            if request.output != ToolOutput::Panel {
                record.severity = git_engine::Severity::Success;
            }
        }
        let run = ToolRun {
            record: record.id,
            exit_code: record.exit_code,
            presentation,
        };
        let failure = if record.exit_code.is_some_and(|code| code != 0)
            || (record.exit_code.is_none() && !stop.is_stopped())
        {
            record.answered = true;
            Some(GitError::from(record.clone()))
        } else {
            None
        };
        (self.command_sink(repo))(record);
        failure.map_or(Ok(run), Err)
    }
}

fn output_tail(stdout: &str, stderr: &str) -> String {
    let mut lines: Vec<_> = stdout
        .trim_end()
        .lines()
        .chain(stderr.trim_end().lines())
        .rev()
        .take(8)
        .collect();
    lines.reverse();
    let joined = lines.join("\n");
    let start = joined.len().saturating_sub(4096);
    joined[joined.ceil_char_boundary(start)..].to_owned()
}

#[derive(Serialize)]
struct ExpandedTool {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
}

impl ExpandedTool {
    fn command<'a>(&'a self, name: &'a str) -> UserCommand<'a> {
        UserCommand {
            name,
            program: &self.program,
            args: &self.args,
            cwd: &self.cwd,
        }
    }

    fn confirmation(&self, root: &Path, request: &ToolRequest) -> Result<String, GitError> {
        let data =
            serde_json::to_vec(&(root, &request.name, request.detached, &request.source, self))
                .map_err(|err| invalid(err.to_string()))?;
        Ok(digest(&data))
    }
}
