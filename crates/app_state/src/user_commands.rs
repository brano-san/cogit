//! User commands (F-954): words split, then placeholders filled; no shell, no environment.

use crate::desktop::{Platform, native_path};
use crate::location::RepoLocation;
use crate::merge_tool::tokenize;
use crate::{AppState, RepoId};
use git_engine::{GitError, NetworkStop, UserCommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolContext {
    /// Relative to the root, `/`-separated.
    pub file: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequest {
    pub name: String,
    pub program: String,
    pub args: String,
    pub cwd: String,
    /// Started and left to run, output unseen (a GUI tool); else captured into the journal.
    pub detached: bool,
    pub context: ToolContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolRun {
    pub record: u32,
    /// `None` when the user stopped it; a detached run that started is `0`.
    pub exit_code: Option<i32>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ToolArgsError {
    #[error("the program is empty")]
    EmptyProgram,
    #[error("unbalanced quote in the arguments")]
    UnbalancedQuote,
    #[error(
        "unknown placeholder `{0}`; use ${{repo}}, ${{file}}, ${{relfile}}, ${{commit}}, ${{shortcommit}} or ${{branch}}"
    )]
    Unknown(String),
    #[error("`{0}` has nothing to stand for here")]
    Missing(String),
}

const SHORT_COMMIT: usize = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values {
    pub repo: String,
    pub file: Option<String>,
    pub relfile: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
}

impl Values {
    #[must_use]
    pub fn of(platform: Platform, root: &Path, context: &ToolContext) -> Self {
        let root = root.to_string_lossy().replace('\\', "/");
        let root = root.trim_end_matches('/');
        let relfile = context
            .file
            .as_deref()
            .map(|file| file.trim_start_matches('/'))
            .filter(|file| !file.is_empty());
        Self {
            repo: native_path(platform, root),
            file: relfile.map(|file| native_path(platform, &format!("{root}/{file}"))),
            relfile: relfile.map(str::to_owned),
            commit: context.commit.clone().filter(|commit| !commit.is_empty()),
            branch: context.branch.clone().filter(|branch| !branch.is_empty()),
        }
    }

    fn get(&self, name: &str) -> Result<Option<String>, ToolArgsError> {
        Ok(match name {
            "repo" => Some(self.repo.clone()),
            "file" => self.file.clone(),
            "relfile" => self.relfile.clone(),
            "commit" => self.commit.clone(),
            "shortcommit" => self
                .commit
                .as_ref()
                .map(|commit| commit.chars().take(SHORT_COMMIT).collect()),
            "branch" => self.branch.clone(),
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
    tokenize(args)
        .map_err(|_| ToolArgsError::UnbalancedQuote)?
        .iter()
        .map(|word| expand_word(word, values))
        .collect()
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
    /// On this computer, not through the agent: WSL is reached by its `\\wsl.localhost\…` path.
    pub fn run_user_command(
        &self,
        repo: RepoId,
        request: &ToolRequest,
        stop: &NetworkStop,
    ) -> Result<ToolRun, GitError> {
        let root = self.root_of(repo)?;
        if matches!(RepoLocation::of(&root), RepoLocation::Ssh { .. }) {
            return Err(GitError::InvalidState(SSH_REFUSAL.to_owned()));
        }
        let values = Values::of(Platform::current(), &root, &request.context);
        let (program, args, cwd) = prepare(request, &values)
            .map_err(|err| GitError::InvalidState(format!("{}: {err}", request.name)))?;
        let command = UserCommand {
            name: &request.name,
            program: &program,
            args: &args,
            cwd: &cwd,
        };
        let record = if request.detached {
            command.start()?;
            let mut record =
                git_engine::GitOutput::record(&root, command.line(), Some(0), "", "", 0);
            record.operation = format!("Tool: {}", request.name);
            record.summary = "Started; it runs on its own".to_owned();
            record
        } else {
            command.run(&root, stop)?
        };
        let run = ToolRun {
            record: record.id,
            exit_code: record.exit_code,
        };
        (self.command_sink(repo))(record);
        Ok(run)
    }
}
