//! Open in External Diff Tool and Open in External Editor (F-944), as set in Preferences.

use crate::desktop::{Launch, Platform, native_path};
use crate::location::RepoLocation;
use crate::merge_tool::tokenize;
use crate::{AppState, RepoId};
use git_engine::{DiffSpec, GitError};

pub const DEFAULT_DIFF_ARGS: &str = "{local} {remote}";
pub const DEFAULT_EDITOR_ARGS: &str = "${file}";

pub const WSL_REFUSAL: &str = "Open in External Diff Tool is not available for a repository inside WSL: git difftool would run inside the distribution, out of reach of Windows programs. Use the Diff panel, or Open in External Editor.";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExternalError {
    #[error("the {tool} program is empty")]
    EmptyProgram { tool: &'static str },
    #[error("unbalanced quote in the {tool} arguments")]
    UnbalancedQuote { tool: &'static str },
    #[error("unknown placeholder `{name}` in the {tool} arguments; use {known}")]
    UnknownPlaceholder {
        tool: &'static str,
        name: String,
        known: &'static str,
    },
}

fn program_of(program: &str, tool: &'static str) -> Result<String, ExternalError> {
    let program = program.trim();
    let program = program
        .strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .unwrap_or(program)
        .trim();
    if program.is_empty() {
        return Err(ExternalError::EmptyProgram { tool });
    }
    Ok(program.to_owned())
}

fn words(args: &str, fallback: &str, tool: &'static str) -> Result<Vec<String>, ExternalError> {
    let line = if args.trim().is_empty() {
        fallback
    } else {
        args
    };
    tokenize(line).map_err(|_| ExternalError::UnbalancedQuote { tool })
}

fn sh_word(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A `difftool.<tool>.cmd` for git's shell: every word quoted, `{local}`, `{remote}` and
/// `{merged}` turned into git's variables. Windows separators in the program become `/`,
/// which git's shell and Windows both read.
pub fn diff_tool_shell_command(
    platform: Platform,
    program: &str,
    args: &str,
) -> Result<String, ExternalError> {
    const TOOL: &str = "diff tool";
    let mut program = program_of(program, TOOL)?;
    if platform == Platform::Windows {
        program = program.replace('\\', "/");
    }
    let mut line = vec![sh_word(&program)];
    for word in words(args, DEFAULT_DIFF_ARGS, TOOL)? {
        let mut out = String::new();
        let mut literal = String::new();
        let mut rest = word.as_str();
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}').map(|at| open + at) else {
                break;
            };
            literal.push_str(&rest[..open]);
            let variable = match &rest[open + 1..close] {
                "local" => "LOCAL",
                "remote" => "REMOTE",
                "merged" => "MERGED",
                other => {
                    return Err(ExternalError::UnknownPlaceholder {
                        tool: TOOL,
                        name: format!("{{{other}}}"),
                        known: "{local}, {remote} and {merged}",
                    });
                }
            };
            if !literal.is_empty() {
                out.push_str(&sh_word(&literal));
                literal.clear();
            }
            out.push_str(&format!("\"${variable}\""));
            rest = &rest[close + 1..];
        }
        literal.push_str(rest);
        if !literal.is_empty() || out.is_empty() {
            out.push_str(&sh_word(&literal));
        }
        line.push(out);
    }
    Ok(line.join(" "))
}

/// `file` is absolute and `/`-separated; no `line` counts as the first.
pub fn editor_command(
    platform: Platform,
    program: &str,
    args: &str,
    file: &str,
    line: Option<u32>,
) -> Result<Launch, ExternalError> {
    const TOOL: &str = "editor";
    let program = program_of(program, TOOL)?;
    let file = native_path(platform, file);
    let line = line.unwrap_or(1).max(1).to_string();
    let mut out = Vec::new();
    for word in words(args, DEFAULT_EDITOR_ARGS, TOOL)? {
        let mut rest = word.as_str();
        while let Some(at) = rest.find("${") {
            let tail = &rest[at..];
            let end = tail.find('}').map_or(tail.len(), |close| close + 1);
            let name = &tail[..end];
            if name != "${file}" && name != "${line}" {
                return Err(ExternalError::UnknownPlaceholder {
                    tool: TOOL,
                    name: name.to_owned(),
                    known: "${file} and ${line}",
                });
            }
            rest = &tail[end..];
        }
        out.push(word.replace("${file}", &file).replace("${line}", &line));
    }
    Ok(Launch {
        program,
        args: out,
        hidden: false,
    })
}

impl AppState {
    /// `git difftool` on what the Files pane shows for `path`; an empty `program` leaves the
    /// choice to git's `diff.tool`. Refused for WSL: git would run the tool inside it.
    pub fn start_diff_tool(
        &self,
        repo: RepoId,
        spec: &DiffSpec,
        path: &str,
        old_path: Option<&str>,
        program: &str,
        args: &str,
    ) -> Result<(), GitError> {
        let root = self.root_of(repo)?;
        if self.remote(repo).is_some()
            || matches!(RepoLocation::of(&root), RepoLocation::Wsl { .. })
        {
            return Err(GitError::InvalidState(WSL_REFUSAL.to_owned()));
        }
        let custom = if program.trim().is_empty() {
            None
        } else {
            Some(
                diff_tool_shell_command(Platform::current(), program, args)
                    .map_err(|err| GitError::InvalidState(err.to_string()))?,
            )
        };
        self.handle(repo)?
            .launch_difftool(spec, path, old_path, custom.as_deref())
    }
}
