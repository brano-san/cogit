mod ancestry;
mod apply;
mod blame;
mod blame_origins;
mod blobs;
mod branches;
mod bypass;
pub mod children;
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
mod gitlink;
mod graph_walk;
mod health;
mod history;
mod hook_run;
mod hooks;
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
mod replay;
mod repo;
mod repo_settings;
mod reset;
mod runner;
mod search;
mod shared;
mod side_modes;
mod staging;
mod stash;
mod stash_rename;
mod state;
mod status;
mod submodules;
mod subtrees;
mod surgery;
mod tags;
mod topo;
mod worktree;
mod worktrees;

pub use blame::BlameLine;
pub use blame_origins::{
    BlameCommit, BlameReport, BlameSource, LineChange, OriginLine, PreviousFile,
};
pub use blobs::{DiffAttributes, DiffSides, DiffSpec};
pub use branches::{CheckoutTarget, RemoteDeletion};
pub use bypass::Bypass;
pub use commit::{CommitDetails, DEFAULT_SIMILARITY, FileEntry, FileMode, FileStatus, Signature};
pub use commit_write::CommitRequest;
pub use config_file::{
    ConfigFile, ConfigProblem, ConfigScope, origin_file, read_config, save_config,
    user_config_by_rules, user_config_path,
};
pub use conflicts::{ConflictSide, ConflictSides, ConflictText};
pub use error::{GitCommandError, GitError};
pub use file_log::{FileChange, FileRevision};
pub use file_ops::{IndexEditorSides, IndexFlag};
pub use find::{Found, FoundKind};
pub use flow::{FlowBranch, FlowConfig, FlowKind, FlowStatus};
pub use gitlink::{ModuleProblem, is_foreign_path};
pub use graph_walk::{CutParents, Reuse, WalkedHistory};
pub use health::{HealthFinding, HealthIssue, case_sensitive};
pub use history::{CommitRow, CommitText};
pub use hook_run::HookRun;
pub use hooks::{Hook, HookOverview, HookSource, HookState, is_hook_name};
pub use interactive::{TodoAction, TodoEntry, render_todo, render_todo_paused};
pub use lfs::{LfsOp, lfs_version, lfs_version_from};
pub use line_history::LineVersion;
pub use listing::{
    BATCH, ContentMatch, MAX_SEARCH_BYTES, PREVIEW_CHARS, SearchRequest, SearchScope,
};
pub use mailmap::Mailmap;
pub use merging::MergeOptions;
pub use module_ops::SubmoduleOp;
pub use network::{NetworkStop, auth_config, auth_header, wants_auth};
pub use operations::RebaseOptions;
pub use origin_search::{
    DeeperTarget, Likelihood, LineMatch, OriginCandidate, OriginKind, OriginQuery, OriginReport,
    OriginText,
};
pub use outcome::Severity;
pub use overlap::{Overlap, OverlapRow, overlap_of, shared_paths};
pub use presets::{Preset, PresetTool, builtin_presets, find_tool, parse_preset, preset_toml};
pub use progress::{RebaseProgress, RebaseStep};
pub use pulse::{RepoPulse, pulse};
pub use ref_meta::RefDate;
pub use reflog::{Reachable, ReflogEntry};
pub use repo::{Branch, BranchKind, Head, RepoHandle, Tag};
pub use repo_settings::{REPO_SETTING_KEYS, RepoSetting, RepoSettingChange};
pub use reset::ResetMode;
pub use runner::{
    CommandSink, GitOutput, git_version, gix_version, redact_command, use_git_program,
};
pub use search::{CommitQuery, GraphRows, GraphView, SkippedRef};
pub use shared::SharedRepo;
pub use stash::{StashContents, StashEntry, StashOptions};
pub use state::RepoState;
pub use status::{RepoStatus, WorkingState};
pub use submodules::{Submodule, SubmodulePointer, SubmoduleState};
pub use subtrees::SubtreeOp;
pub use tags::TagRequest;
pub use worktree::{WorktreeFiles, WorktreeView};
pub use worktrees::WorktreeEntry;

pub type Result<T> = std::result::Result<T, GitError>;

pub use apply::PatchTarget;
pub use find::InvestigationStep;
