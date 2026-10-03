//! Everything the Conflict Solver window is given for one file, in one read.

use crate::{AppState, RepoId, merge_tool};
use git_engine::{ConflictContext, GitError};
use serde::Serialize;

/// Past this a side is not merged line by line: the solver offers to take a side whole.
pub const MAX_SOLVER_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SolverData {
    pub context: ConflictContext,
    pub binary: bool,
    /// A link or a submodule: its sides are taken whole and labeled as what they are.
    pub kind: git_engine::EntryKind,
    pub too_large: bool,
    /// Ours or theirs lacks the file: it was deleted there (modify/delete).
    pub missing_ours: bool,
    pub missing_theirs: bool,
    pub has_base: bool,
    /// The index entries these sides were read from: Save names them back.
    pub stages: git_engine::ConflictStages,
    pub crlf: bool,
    /// LF-normalized text of the sides; `None` for a side without the file, and for every
    /// side of a binary or oversized file.
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub regions: Vec<diff_engine::SolverRegion>,
}

fn is_crlf(sides: &[&Option<Vec<u8>>]) -> bool {
    let like = sides.iter().find_map(|side| side.as_deref());
    like.is_some_and(|bytes| {
        let crlf = bytes.windows(2).filter(|pair| *pair == b"\r\n").count();
        let lf = bytes.iter().filter(|&&byte| byte == b'\n').count() - crlf;
        crlf > lf
    })
}

impl AppState {
    pub fn solver_data(&self, repo: RepoId, path: &str) -> Result<SolverData, GitError> {
        let handle = self.handle(repo)?;
        let sides = handle.conflict_sides(path)?;
        let context = handle.conflict_context(path);
        let missing_ours = sides.ours.is_none();
        let missing_theirs = sides.theirs.is_none();
        let crlf = is_crlf(&[&sides.ours, &sides.theirs, &sides.base]);
        let largest = [&sides.base, &sides.ours, &sides.theirs]
            .into_iter()
            .flatten()
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        let binary = !sides.is_text();
        let too_large = largest > MAX_SOLVER_BYTES;
        let mut data = SolverData {
            context,
            binary,
            kind: sides.kind,
            too_large,
            missing_ours,
            missing_theirs,
            has_base: sides.base.is_some(),
            stages: sides.stages.clone(),
            crlf,
            base: None,
            ours: None,
            theirs: None,
            regions: Vec::new(),
        };
        if binary || too_large {
            return Ok(data);
        }
        let text = sides.to_text();
        let normal = |side: Option<String>| {
            side.map(|text| diff_engine::normalize_line_endings(&text).into_owned())
        };
        data.base = normal(text.base);
        data.ours = normal(text.ours);
        data.theirs = normal(text.theirs);
        if !missing_ours && !missing_theirs {
            data.regions = diff_engine::merge3_sides(
                data.base.as_deref().unwrap_or_default(),
                data.ours.as_deref().unwrap_or_default(),
                data.theirs.as_deref().unwrap_or_default(),
                diff_engine::merge_grammar_for_path(path),
            );
        }
        Ok(data)
    }

    /// The working file settled outside the solver (an external tool, a hand edit): it is
    /// taken as it is and staged, the file kept for Undo first.
    pub fn mark_conflict_resolved(&self, repo: RepoId, path: &str) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let kept = handle
            .keep_worktree_file(path)
            .map_err(|err| crate::backup_failed("marking", &err))?;
        handle.mark_resolved(path)?;
        self.record_resolution(repo, format!("Mark {path} resolved"), path, kept);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeToolOutcome {
    pub exit_code: Option<i32>,
    pub canceled: bool,
    pub markers_left: bool,
    pub conflicted: bool,
}

fn refused(err: &merge_tool::ToolError) -> GitError {
    GitError::InvalidState(err.to_string())
}

impl AppState {
    /// Launches the external tool on a copy of the three sides; the tool writes the working
    /// file itself, as `git mergetool` has it do. `program` empty falls back to git's
    /// `merge.tool`. `on_exit` runs on the waiter thread, after the temp files are removed.
    pub fn start_merge_tool(
        &self,
        repo: RepoId,
        path: &str,
        program: &str,
        args: &str,
        temp_root: &std::path::Path,
        on_exit: impl FnOnce(MergeToolOutcome) + Send + 'static,
    ) -> Result<(), GitError> {
        let handle = self.handle(repo)?;
        let sides = handle.conflict_sides(path)?;
        // The tool would write its result over the submodule's folder.
        if sides.kind == git_engine::EntryKind::Submodule {
            return Err(GitError::InvalidState(format!(
                "{path} is a submodule: take ours or theirs, a merge tool cannot merge it"
            )));
        }
        let (program, args) = if program.trim().is_empty() {
            let config = handle.merge_tool_config()?;
            merge_tool::from_git_config(&merge_tool::GitToolConfig {
                tool: config.tool,
                cmd: config.cmd,
                path: config.path,
            })
            .map_err(|err| refused(&err))?
        } else {
            (program.to_owned(), args.to_owned())
        };
        let files = merge_tool::prepare(
            temp_root,
            handle.root(),
            path,
            sides.base.as_deref(),
            sides.ours.as_deref(),
            sides.theirs.as_deref(),
        )
        .map_err(|err| refused(&err.into()))?;
        let command = match merge_tool::build_command(&program, &args, &files) {
            Ok(command) => command,
            Err(err) => {
                files.cleanup();
                return Err(refused(&err));
            }
        };
        let key = merge_tool::ToolKey {
            repo: repo.0,
            path: path.to_owned(),
        };
        let owned = path.to_owned();
        let root = handle.root().to_path_buf();
        let cwd = root.clone();
        self.merge_tools
            .start(key, command, &cwd, files, move |exit| {
                let markers_left = std::fs::read(root.join(&owned))
                    .map(|bytes| merge_tool::has_conflict_markers(&String::from_utf8_lossy(&bytes)))
                    .unwrap_or(false);
                let conflicted = handle
                    .conflicted_paths()
                    .map(|paths| paths.contains(&owned))
                    .unwrap_or(true);
                on_exit(MergeToolOutcome {
                    exit_code: exit.code,
                    canceled: exit.canceled,
                    markers_left,
                    conflicted,
                });
            })
            .map_err(|err| refused(&err))
    }

    pub fn cancel_merge_tool(&self, repo: RepoId, path: &str) -> bool {
        self.merge_tools.cancel(&merge_tool::ToolKey {
            repo: repo.0,
            path: path.to_owned(),
        })
    }

    #[must_use]
    pub fn merge_tools_running(&self, repo: RepoId) -> Vec<String> {
        self.merge_tools
            .running()
            .into_iter()
            .filter(|key| key.repo == repo.0)
            .map(|key| key.path)
            .collect()
    }
}
