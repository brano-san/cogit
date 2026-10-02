mod ancestry;
mod apply;
mod bisect;
mod blame;
mod blame_origins;
mod blobs;
mod branches;
mod bypass;
pub mod children;
mod cloning;
mod commit;
mod commit_write;
mod config_file;
mod conflicts;
pub mod discover;
mod error;
mod file_log;
mod file_ops;
mod find;
mod flow;
mod git_candidates;
mod git_probe;
mod gitlink;
mod graph_walk;
mod health;
mod history;
mod hook_run;
mod hooks;
mod housekeeping;
mod init;
mod interactive;
mod lfs;
mod line_history;
mod line_match;
mod listing;
mod mailmap;
mod maintenance;
mod merging;
mod module_ops;
mod network;
mod network_options;
mod notes;
mod operations;
mod origin_search;
pub mod outcome;
pub mod output_text;
mod overlap;
pub mod phases;
mod presets;
mod progress;
mod published;
mod pulse;
mod ref_meta;
mod reflog;
mod remotes;
mod replay;
mod repo;
mod repo_settings;
mod rerere;
mod reset;
mod revision;
mod runner;
mod search;
mod shared;
mod side_modes;
mod solver;
mod staging;
mod stash;
mod stash_rename;
mod state;
mod status;
mod submodules;
mod subtrees;
mod surgery;
mod tags;
mod text_search;
mod textconv;
mod topo;
mod worktree;
mod worktree_scan;
mod worktrees;

pub use bisect::{BisectMark, BisectState, BisectTerms};
pub use blame::BlameLine;
pub use blame_origins::{
    BlameCommit, BlameReport, BlameSource, LineChange, OriginLine, PreviousFile,
};
pub use blobs::{DiffAttributes, DiffContent, DiffSides, DiffSpec, MAX_HASHED_BYTES};
pub use branches::{BranchDeletion, CheckoutTarget, RemoteDeletion, windows_forbidden};
pub use bypass::Bypass;
pub use cloning::{
    CloneDestination, CloneRequest, Login, RemoteBranches, clone_destination, clone_repository,
    remote_branches, repository_url_in,
};
pub use commit::{
    CommitDetails, ConflictKind, DEFAULT_SIMILARITY, FileEntry, FileMode, FileStatus, Signature,
    SignatureCheck, SubmoduleChange, Trailer,
};
pub use commit_write::CommitRequest;
pub use config_file::{
    ConfigFile, ConfigProblem, ConfigScope, origin_file, read_config, save_config,
    user_config_by_rules, user_config_path,
};
pub use conflicts::{ConflictSide, ConflictSides, ConflictText};
pub use error::{GitCommandError, GitError};
pub use file_log::{FileChange, FileRevision};
pub use file_ops::{IgnoreRule, IndexEditorSides, IndexFlag};
pub use find::{Found, FoundKind};
pub use flow::{FlowBranch, FlowConfig, FlowKind, FlowStatus};
pub use git_candidates::{GitCandidate, MIN_GIT, find_git_candidates, is_below_min_git};
pub use git_probe::{GitProbe, parse_git_version, probe_git};
pub use gitlink::{ModuleProblem, is_foreign_path};
pub use graph_walk::{CutParents, Reuse, WalkedHistory};
pub use health::{HealthFinding, HealthIssue, case_sensitive};
pub use history::{CommitRow, CommitText};
pub use hook_run::HookRun;
pub use hooks::{Hook, HookOverview, HookSource, HookState, is_hook_name};
pub use housekeeping::MaintenanceTask;
pub use init::{FolderKind, folder_kind, init_repository};
pub use interactive::{TodoAction, TodoEntry, render_todo, render_todo_paused};
pub use lfs::{LfsFileState, LfsLock, LfsOp, lfs_version, lfs_version_from};
pub use line_history::LineVersion;
pub use listing::{
    BATCH, ContentMatch, MAX_SEARCH_BYTES, PREVIEW_CHARS, SearchRequest, SearchScope,
};
pub use mailmap::Mailmap;
pub use merging::MergeOptions;
pub use module_ops::SubmoduleOp;
pub use network::{NetworkStop, auth_config, auth_header, wants_auth};
pub use network_options::{
    FetchOptions, NetworkDefaults, NotesFetch, PullMethod, PullOptions, PushCommit, PushOptions,
    PushOutcome, PushPreview, TagsMode,
};
pub use notes::CommitNote;
pub use operations::RebaseOptions;
pub use origin_search::{
    DeeperTarget, Likelihood, LineMatch, OriginCandidate, OriginKind, OriginQuery, OriginReport,
    OriginText,
};
pub use outcome::Severity;
pub use overlap::{Overlap, OverlapRow, overlap_of, shared_paths};
pub use presets::{
    Preset, PresetTool, builtin_presets, find_tool, parse_preset, preset_toml, tool_search_places,
};
pub use progress::{RebaseProgress, RebaseStep};
pub use pulse::{RepoPulse, pulse};
pub use ref_meta::{OtherRef, RefDate};
pub use reflog::{Reachable, ReflogEntry};
pub use remotes::RemoteInfo;
pub use repo::{Branch, BranchKind, Head, RepoHandle, Tag};
pub use repo_settings::{REPO_SETTING_KEYS, RepoSetting, RepoSettingChange};
pub use rerere::RerereStatus;
pub use reset::ResetMode;
pub use revision::{CommitPreview, RevisionCheck};
pub use runner::{
    CommandSink, GitOutput, git_version, gix_version, redact_command, use_git_program,
};
pub use search::{CommitQuery, GraphRows, GraphView, Passed, PassedCommits, ShownBy, SkippedRef};
pub use shared::SharedRepo;
pub use solver::{
    ConflictContext, ConflictOperation, MergeToolConfig, marker_labels, merge_target,
    merge_tool_from_listing,
};
pub use stash::{AutostashOutcome, StashContents, StashEntry, StashOptions};
pub use state::RepoState;
pub use status::{RepoStatus, WorkingState};
pub use submodules::{Submodule, SubmodulePointer, SubmoduleState, resolve_relative_url};
pub use subtrees::SubtreeOp;
pub use tags::TagRequest;
pub use text_search::TextFields;
pub use worktree::{WorktreeFiles, WorktreeView};
pub use worktree_scan::{UnpushedInSubmodule, WorktreeScan, WorktreeSubmodules};
pub use worktrees::{WorktreeBranch, WorktreeEntry, worktree_folder_problem};

pub type Result<T> = std::result::Result<T, GitError>;

pub use apply::PatchTarget;
pub use find::InvestigationStep;

/// A path as it crosses IPC: `/` on every platform.
#[must_use]
pub fn slash_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::slash_path;
    use std::path::Path;

    #[test]
    fn every_backslash_becomes_a_slash_and_nothing_else_changes() {
        let cases = [
            (r"C:\Users\Аня\repo", "C:/Users/Аня/repo"),
            (r"\\?\C:\x", "//?/C:/x"),
            (r"\\?\UNC\srv\share", "//?/UNC/srv/share"),
            ("/home/ann/repo", "/home/ann/repo"),
        ];
        for (path, shown) in cases {
            assert_eq!(slash_path(Path::new(path)), shown);
        }
    }
}
