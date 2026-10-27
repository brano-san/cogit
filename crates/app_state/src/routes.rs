use crate::backend::{Request, answer};
use crate::{AppState, RepoId};
use git_engine::{GitError, NetworkStop};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::DiffBatch;
use crate::FileContent;
use crate::RepoRefs;
use crate::SolverData;
use crate::UndoRewrite;
use git_engine::BisectMark;
use git_engine::BlameReport;
use git_engine::FileRevision;
use git_engine::IndexEditorSides;
use git_engine::IndexFlag;
use git_engine::LfsOp;
use git_engine::NetworkDefaults;
use git_engine::PushOptions;
use git_engine::PushPreview;
use git_engine::RemoteInfo;
use git_engine::RepoSetting;
use git_engine::RepoSettingChange;
use git_engine::ResetMode;
use git_engine::SubmoduleOp;
use git_engine::SubtreeOp;
use std::borrow::Borrow;
use std::path::{Path, PathBuf};

macro_rules! routed {
    ($(fn $name:ident($($args:tt)*) -> $answer:ty = $local:ident;)*) => {
        #[allow(non_camel_case_types)]
        #[derive(Debug, Serialize, Deserialize)]
        pub(crate) enum RepoCall {
            $($name($name),)*
        }

        impl RepoCall {
            pub(crate) fn run(self, state: &AppState, stop: &NetworkStop) -> Result<Value, Value> {
                match self {
                    $(Self::$name(call) => $name::run(call, state, stop),)*
                }
            }

            pub(crate) fn name(&self) -> &'static str {
                match self {
                    $(Self::$name(_) => stringify!($name),)*
                }
            }
        }

        $(route!(@munch [call stop] $name $local [$answer] [] [] [] [] [] [None]; $($args)*);)*
    };
}

macro_rules! route {
    (@munch [$call:ident $stopv:ident] $name:ident $local:ident [$answer:ty] [$($p:tt)*] [$($f:tt)*] [$($o:tt)*] [$($l:tt)*] [$($c:tt)*] [$stop:expr];
        $n:ident: &NetworkStop $(, $($rest:tt)*)?) => {
        route!(@munch [$call $stopv] $name $local [$answer] [$($p)* $n: &NetworkStop,] [$($f)*] [$($o)*] [$($l)* $stopv,] [$($c)* $n,] [Some($n)]; $($($rest)*)?);
    };
    (@munch [$call:ident $stopv:ident] $name:ident $local:ident [$answer:ty] [$($p:tt)*] [$($f:tt)*] [$($o:tt)*] [$($l:tt)*] [$($c:tt)*] [$stop:expr];
        $n:ident: Option<&$t:ty> $(, $($rest:tt)*)?) => {
        route!(@munch [$call $stopv] $name $local [$answer]
            [$($p)* $n: Option<&$t>,]
            [$($f)* $n: Option<<$t as ToOwned>::Owned>,]
            [$($o)* $n: $n.map(ToOwned::to_owned),]
            [$($l)* $call.$n.as_ref().map(<<$t as ToOwned>::Owned as Borrow<$t>>::borrow),]
            [$($c)* $n,] [$stop]; $($($rest)*)?);
    };
    (@munch [$call:ident $stopv:ident] $name:ident $local:ident [$answer:ty] [$($p:tt)*] [$($f:tt)*] [$($o:tt)*] [$($l:tt)*] [$($c:tt)*] [$stop:expr];
        $n:ident: &$t:ty $(, $($rest:tt)*)?) => {
        route!(@munch [$call $stopv] $name $local [$answer]
            [$($p)* $n: &$t,]
            [$($f)* $n: <$t as ToOwned>::Owned,]
            [$($o)* $n: $n.to_owned(),]
            [$($l)* <<$t as ToOwned>::Owned as Borrow<$t>>::borrow(&$call.$n),]
            [$($c)* $n,] [$stop]; $($($rest)*)?);
    };
    (@munch [$call:ident $stopv:ident] $name:ident $local:ident [$answer:ty] [$($p:tt)*] [$($f:tt)*] [$($o:tt)*] [$($l:tt)*] [$($c:tt)*] [$stop:expr];
        $n:ident: $t:ty $(, $($rest:tt)*)?) => {
        route!(@munch [$call $stopv] $name $local [$answer]
            [$($p)* $n: $t,] [$($f)* $n: $t,] [$($o)* $n,] [$($l)* $call.$n,] [$($c)* $n,] [$stop]; $($($rest)*)?);
    };
    (@munch [$call:ident $stopv:ident] $name:ident $local:ident [$answer:ty] [$($p:tt)*] [$($f:tt)*] [$($o:tt)*] [$($l:tt)*] [$($c:tt)*] [$stop:expr];) => {
        #[allow(non_camel_case_types)]
        #[derive(Debug, Serialize, Deserialize)]
        pub(crate) struct $name {
            repo: RepoId,
            $($f)*
        }

        impl $name {
            #[allow(unused_variables)]
            fn run($call: Self, state: &AppState, $stopv: &NetworkStop) -> Result<Value, Value> {
                answer(state.$local($call.repo, $($l)*))
            }
        }

        impl AppState {
            pub fn $name(&self, repo: RepoId, $($p)*) -> $answer {
                match self.remote(repo) {
                    None => self.$local(repo, $($c)*),
                    Some(remote) => remote.ask(
                        &Request::Repo(RepoCall::$name($name { repo: remote.id(), $($o)* })),
                        $stop,
                    ),
                }
            }
        }
    };
}

routed! {
    fn repo_status() -> Result<git_engine::RepoStatus, GitError> = local_repo_status;
    fn worktree_files(view: git_engine::WorktreeView) -> Result<git_engine::WorktreeFiles, GitError> = local_worktree_files;
    fn repo_refs() -> Result<RepoRefs, GitError> = local_repo_refs;
fn bisect_start(bad: &str, good: Option<&str>) -> Result<(), GitError> = local_bisect_start;
    fn bisect_mark(mark: BisectMark, rev: Option<&str>) -> Result<(), GitError> = local_bisect_mark;
    fn bisect_reset() -> Result<(), GitError> = local_bisect_reset;
    fn checkout(target: &git_engine::CheckoutTarget) -> Result<(), git_engine::GitError> = local_checkout;
    fn create_branch(name: &str, start: Option<&str>, switch: bool) -> Result<(), git_engine::GitError> = local_create_branch;
    fn rename_branch(from: &str, to: &str, force: bool) -> Result<(), git_engine::GitError> = local_rename_branch;
    fn set_upstream(branch: &str, upstream: Option<&str>) -> Result<(), git_engine::GitError> = local_set_upstream;
    fn delete_branch(name: &str, force: bool) -> Result<git_engine::BranchDeletion, git_engine::GitError> = local_delete_branch;
    fn create_tag(request: &git_engine::TagRequest) -> Result<(), git_engine::GitError> = local_create_tag;
    fn delete_tag(name: &str) -> Result<(), git_engine::GitError> = local_delete_tag;
    fn run_maintenance(task: git_engine::MaintenanceTask, prune: git_engine::GcPrune) -> Result<(), GitError> = local_run_maintenance;
    fn add_to_exclude(paths: &[String]) -> Result<(), GitError> = local_add_to_exclude;
    fn ignore_rules(paths: &[String]) -> Result<Vec<git_engine::IgnoreRule>, GitError> = local_ignore_rules;
    fn lfs_locks() -> Result<Vec<git_engine::LfsLock>, GitError> = local_lfs_locks;
    fn lfs_file_states(paths: &[String]) -> Result<Vec<git_engine::LfsFileState>, GitError> = local_lfs_file_states;
    fn commit_signature(rev: &str) -> Result<git_engine::SignatureCheck, GitError> = local_commit_signature;
    fn commit_signatures(oids: &[String]) -> Result<Vec<git_engine::SignatureStatus>, GitError> = local_commit_signatures;
    fn unportable_paths(rev: &str) -> Result<Vec<String>, GitError> = local_unportable_paths;
    fn rerere_status() -> Result<git_engine::RerereStatus, GitError> = local_rerere_status;
    fn rerere_forget(paths: &[String]) -> Result<(), GitError> = local_rerere_forget;
    fn range_diff(before: &str, after: &str) -> Result<git_engine::GitOutput, GitError> = local_range_diff;
    fn remote_url(name: &str) -> Result<Option<String>, git_engine::GitError> = local_remote_url;
    fn health() -> Result<Vec<git_engine::HealthFinding>, git_engine::GitError> = local_health;
    fn trust_directory() -> Result<(), git_engine::GitError> = local_trust_directory;
    fn conflicted_paths() -> Result<Vec<String>, git_engine::GitError> = local_conflicted_paths;
    fn conflict_text(path: &str) -> Result<git_engine::ConflictText, git_engine::GitError> = local_conflict_text;
    fn resolve_conflict(path: &str, side: git_engine::ConflictSide, expected: Option<&git_engine::ConflictStages>) -> Result<(), git_engine::GitError> = local_resolve_conflict;
    fn resolve_conflict_text(path: &str, text: &str, expected: Option<&git_engine::ConflictStages>) -> Result<(), git_engine::GitError> = local_resolve_conflict_text;
    fn save_conflict_text(path: &str, text: &str, expected: Option<&git_engine::ConflictStages>) -> Result<(), git_engine::GitError> = local_save_conflict_text;
    fn diff_file(spec: &git_engine::DiffSpec, path: &str, options: &diff_engine::DiffOptions) -> Result<diff_engine::FileDiff, git_engine::GitError> = local_diff_file;
    fn diff_edited(spec: &git_engine::DiffSpec, path: &str, text: &str, shape: git_engine::editable::Shape, options: &diff_engine::DiffOptions) -> Result<diff_engine::FileDiff, git_engine::GitError> = local_diff_edited;
    fn file_content(spec: &git_engine::DiffSpec, path: &str, limit: Option<u64>) -> Result<FileContent, git_engine::GitError> = local_file_content;
    fn diff_files(spec: &git_engine::DiffSpec, paths: &[String], options: &diff_engine::DiffOptions, request: u32) -> Result<DiffBatch, git_engine::GitError> = local_diff_files;
    fn stage_selection(request: &diff_engine::PatchRequest, reverse: bool) -> Result<(), git_engine::GitError> = local_stage_selection;
    fn discard_selection(request: &diff_engine::PatchRequest) -> Result<(), git_engine::GitError> = local_discard_selection;
    fn merge_preview(path: &str) -> Result<Vec<diff_engine::Region>, git_engine::GitError> = local_merge_preview;
    fn image_sides(spec: &git_engine::DiffSpec, path: &str) -> Result<(Option<String>, Option<String>), git_engine::GitError> = local_image_sides;
    fn remove_from_repository(paths: &[String], delete_local: bool) -> Result<(), GitError> = local_remove_from_repository;
    fn move_path(from: &str, to: &str) -> Result<(), GitError> = local_move_path;
    fn set_index_flag(paths: &[String], flag: IndexFlag, on: bool) -> Result<(), GitError> = local_set_index_flag;
    fn read_editable(spec: &git_engine::DiffSpec, path: &str) -> Result<git_engine::editable::EditableFile, GitError> = local_read_editable;
    fn save_editable(path: &str, text: &str, shape: git_engine::editable::Shape, stamp: &str, force: bool) -> Result<git_engine::editable::SaveOutcome, GitError> = local_save_editable;
    fn index_editor_sides(path: &str) -> Result<IndexEditorSides, GitError> = local_index_editor_sides;
    fn write_index_editor(path: &str, index: Option<&str>, worktree: Option<&str>) -> Result<(), GitError> = local_write_index_editor;
    fn save_blob(rev: &str, path: &str, target: &Path) -> Result<(), GitError> = local_save_blob;
    fn export_read_only(rev: &str, path: &str, dir: &Path) -> Result<PathBuf, GitError> = local_export_read_only;
    fn apply_commit_file(rev: &str, path: &str, old_path: Option<&str>, reverse: bool) -> Result<(), GitError> = local_apply_commit_file;
    fn present_on_disk(paths: &[String]) -> Result<Vec<String>, GitError> = local_present_on_disk;
    fn flow_status() -> Result<git_engine::FlowStatus, git_engine::GitError> = local_flow_status;
    fn flow_init(config: &git_engine::FlowConfig) -> Result<(), git_engine::GitError> = local_flow_init;
    fn flow_start(kind: git_engine::FlowKind, name: &str) -> Result<String, git_engine::GitError> = local_flow_start;
    fn flow_finish(kind: git_engine::FlowKind, name: &str, tag: Option<&str>) -> Result<(), git_engine::GitError> = local_flow_finish;
    fn commit_details(rev: &str) -> Result<git_engine::CommitDetails, git_engine::GitError> = local_commit_details;
    fn commit_files(rev: &str) -> Result<Vec<git_engine::FileEntry>, git_engine::GitError> = local_commit_files;
    fn tree_files(rev: &str) -> Result<Vec<String>, git_engine::GitError> = local_tree_files;
    fn file_count(rev: Option<&str>) -> Result<u32, git_engine::GitError> = local_file_count;
    fn find(query: &str, limit: u32) -> Result<Vec<git_engine::Found>, git_engine::GitError> = local_find;
    fn ref_dates() -> Result<Vec<git_engine::RefDate>, git_engine::GitError> = local_ref_dates;
    fn other_refs() -> Result<Vec<git_engine::OtherRef>, git_engine::GitError> = local_other_refs;
    fn lost_commits(limit: u32) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> = local_lost_commits;
    fn blame(path: &str, rev: &str) -> Result<Vec<git_engine::BlameLine>, git_engine::GitError> = local_blame;
    fn line_history(path: &str, rev: &str, line: u32, limit: usize) -> Result<Vec<git_engine::LineVersion>, git_engine::GitError> = local_line_history;
    fn file_revisions(path: &str, rev: &str, limit: usize) -> Result<Vec<git_engine::CommitRow>, git_engine::GitError> = local_file_revisions;
    fn investigate(path: &str, from: u32, to: u32, limit: u32) -> Result<Vec<git_engine::InvestigationStep>, git_engine::GitError> = local_investigate;
    fn hooks() -> Result<git_engine::HookOverview, git_engine::GitError> = local_hooks;
    fn read_hook(name: &str) -> Result<String, git_engine::GitError> = local_read_hook;
    fn write_hook(name: &str, body: &str) -> Result<(), git_engine::GitError> = local_write_hook;
    fn set_hook_enabled(name: &str, enabled: bool) -> Result<(), git_engine::GitError> = local_set_hook_enabled;
    fn use_hooks_path(path: &str) -> Result<(), git_engine::GitError> = local_use_hooks_path;
    fn adopt_hooks(path: &str) -> Result<(), git_engine::GitError> = local_adopt_hooks;
    fn run_hook(name: &str, stop: &NetworkStop) -> Result<git_engine::HookRun, git_engine::GitError> = local_run_hook;
    fn bypass_log() -> Result<Vec<git_engine::Bypass>, git_engine::GitError> = local_bypass_log;
    fn commit_template() -> Result<Option<String>, git_engine::GitError> = local_commit_template;
    fn recent_commits(limit: u32) -> Result<Vec<git_engine::CommitDetails>, git_engine::GitError> = local_recent_commits;
    fn file_log(path: &str, rev: Option<&str>, follow: bool) -> Result<Vec<FileRevision>, GitError> = local_file_log;
    fn blame_origins(path: &str, rev: Option<&str>, ignore_whitespace: bool) -> Result<BlameReport, GitError> = local_blame_origins;
    fn remotes() -> Result<Vec<String>, git_engine::GitError> = local_remotes;
    fn delete_merged_branches() -> Result<Vec<String>, git_engine::GitError> = local_delete_merged_branches;
    fn merge_notes(remote: &str, namespace: &str) -> Result<(), GitError> = local_merge_notes;
    fn push_preview(local: &str, remote: &str, branch: &str, limit: usize) -> Result<PushPreview, GitError> = local_push_preview;
    fn push_tags_preview(options: &PushOptions) -> Result<Vec<String>, GitError> = local_push_tags_preview;
    fn push_atomic_support(options: &PushOptions) -> Result<Option<bool>, GitError> = local_push_atomic_support;
    fn network_defaults() -> Result<NetworkDefaults, GitError> = local_network_defaults;
    fn save_network_defaults(defaults: &NetworkDefaults) -> Result<(), GitError> = local_save_network_defaults;
    fn reset_to(rev: &str, mode: ResetMode) -> Result<(), GitError> = local_reset_to;
    fn undo_rewrite_info() -> Result<Option<UndoRewrite>, GitError> = local_undo_rewrite_info;
    fn undo_rewrite() -> Result<(), GitError> = local_undo_rewrite;
    fn is_ancestor(ancestor: &str, descendant: &str) -> Result<bool, GitError> = local_is_ancestor;
    fn compare_files(from: &str, to: &str) -> Result<Vec<git_engine::FileEntry>, GitError> = local_compare_files;
    fn tag_name_problem(name: &str) -> Result<Option<String>, GitError> = local_tag_name_problem;
    fn tag_message(name: &str) -> Result<Option<String>, GitError> = local_tag_message;
    fn rename_tag(from: &str, to: &str) -> Result<(), GitError> = local_rename_tag;
    fn branch_reflog(branch: &str) -> Result<Vec<git_engine::ReflogEntry>, GitError> = local_branch_reflog;
    fn restore_branch(branch: &str, oid: &str) -> Result<(), GitError> = local_restore_branch;
    fn move_branch(branch: &str, oid: &str) -> Result<(), GitError> = local_move_branch;
    fn noted_commits() -> Result<Vec<String>, GitError> = local_noted_commits;
    fn commit_notes(rev: &str) -> Result<Vec<git_engine::CommitNote>, GitError> = local_commit_notes;
    fn set_note(rev: &str, text: &str) -> Result<(), GitError> = local_set_note;
    fn rename_stash(oid: &str, message: &str) -> Result<(), GitError> = local_rename_stash;
    fn edit_author(rev: &str, name: &str, email: &str) -> Result<(), GitError> = local_edit_author;
    fn submodules_under(parent: &str) -> Result<Vec<git_engine::Submodule>, git_engine::GitError> = local_submodules_under;
    fn update_submodule(path: &str, init: bool) -> Result<(), git_engine::GitError> = local_update_submodule;
    fn remote_info(name: &str) -> Result<RemoteInfo, GitError> = local_remote_info;
    fn rename_remote(from: &str, to: &str) -> Result<(), GitError> = local_rename_remote;
    fn remove_remote(name: &str) -> Result<(), GitError> = local_remove_remote;
    fn set_remote_properties(name: &str, url: &str, background_fetch: bool) -> Result<(), GitError> = local_set_remote_properties;
    fn submodule_op(op: SubmoduleOp, paths: &[String], stop: &NetworkStop) -> Result<(), GitError> = local_submodule_op;
    fn add_submodule(url: &str, path: &str, branch: Option<&str>, stop: &NetworkStop) -> Result<(), GitError> = local_add_submodule;
    fn subtree_op(op: &SubtreeOp, stop: &NetworkStop) -> Result<(), GitError> = local_subtree_op;
    fn subtree_prefixes() -> Result<Vec<String>, GitError> = local_subtree_prefixes;
    fn lfs_op(op: &LfsOp, stop: &NetworkStop) -> Result<(), GitError> = local_lfs_op;
    fn repo_settings() -> Result<Vec<RepoSetting>, GitError> = local_repo_settings;
    fn write_repo_settings(changes: &[RepoSettingChange]) -> Result<(), GitError> = local_write_repo_settings;
    fn abort_operation() -> Result<(), git_engine::GitError> = local_abort_operation;
    fn remove_index_lock() -> Result<(), git_engine::GitError> = local_remove_index_lock;
    fn continue_operation() -> Result<(), git_engine::GitError> = local_continue_operation;
    fn rollback_to(rev: &str, paths: &[String]) -> Result<(), git_engine::GitError> = local_rollback_to;
    fn overlap_window(base: &str, window: &[String]) -> Result<Vec<git_engine::OverlapRow>, git_engine::GitError> = local_overlap_window;
    fn rebase_progress() -> Result<Option<git_engine::RebaseProgress>, git_engine::GitError> = local_rebase_progress;
    fn edit_rebase_todo(steps: &[git_engine::RebaseStep]) -> Result<(), git_engine::GitError> = local_edit_rebase_todo;
    fn rebase_todo(base: &str) -> Result<Vec<git_engine::TodoEntry>, git_engine::GitError> = local_rebase_todo;
    fn interactive_rebase(base: &str, plan: &[git_engine::TodoEntry], paused: bool) -> Result<(), git_engine::GitError> = local_interactive_rebase;
    fn protecting_refs(rev: &str) -> Result<Vec<String>, git_engine::GitError> = local_protecting_refs;
    fn is_published(rev: &str) -> Result<bool, git_engine::GitError> = local_is_published;
    fn unpushed() -> Result<Vec<String>, git_engine::GitError> = local_unpushed;
    fn is_merged_into_head(rev: &str) -> Result<bool, git_engine::GitError> = local_is_merged_into_head;
    fn split_off(rev: &str, paths: &[String], message: &str, split_first: bool) -> Result<(), git_engine::GitError> = local_split_off;
    fn merge(options: &git_engine::MergeOptions) -> Result<(), git_engine::GitError> = local_merge;
    fn rebase(options: &git_engine::RebaseOptions) -> Result<(), git_engine::GitError> = local_rebase;
    fn skip_operation() -> Result<(), git_engine::GitError> = local_skip_operation;
    fn cherry_pick(commits: &[String]) -> Result<(), git_engine::GitError> = local_cherry_pick;
    fn revert(commits: &[String]) -> Result<(), git_engine::GitError> = local_revert;
    fn solver_data(path: &str) -> Result<SolverData, GitError> = local_solver_data;
    fn mark_conflict_resolved(path: &str) -> Result<(), GitError> = local_mark_conflict_resolved;
    fn working_state() -> Result<git_engine::WorkingState, git_engine::GitError> = local_working_state;
    fn stage_paths(paths: &[String]) -> Result<(), git_engine::GitError> = local_stage_paths;
    fn stage_all(files: usize) -> Result<(), git_engine::GitError> = local_stage_all;
    fn unstage_paths(paths: &[String]) -> Result<(), git_engine::GitError> = local_unstage_paths;
    fn discard_paths(paths: &[String]) -> Result<(), git_engine::GitError> = local_discard_paths;
    fn add_to_gitignore(paths: &[String]) -> Result<(), git_engine::GitError> = local_add_to_gitignore;
    fn stage_mode(path: &str, executable: bool) -> Result<(), git_engine::GitError> = local_stage_mode;
    fn commit(request: &git_engine::CommitRequest) -> Result<String, git_engine::GitError> = local_commit;
    fn wants_maintenance() -> Result<bool, git_engine::GitError> = local_wants_maintenance;
    fn maintain_after_commit() -> Result<(), git_engine::GitError> = local_maintain_after_commit;
    fn stashes() -> Result<Vec<git_engine::StashEntry>, git_engine::GitError> = local_stashes;
    fn stash_selection(paths: &[String], message: &str) -> Result<(), git_engine::GitError> = local_stash_selection;
    fn stash_contents(index: u32) -> Result<git_engine::StashContents, git_engine::GitError> = local_stash_contents;
    fn stash_push(options: &git_engine::StashOptions) -> Result<(), git_engine::GitError> = local_stash_push;
    fn stash_keeping_worktree(message: &str) -> Result<(), git_engine::GitError> = local_stash_keeping_worktree;
    fn stash_apply(oid: &str, pop: bool, restore_index: bool) -> Result<(), git_engine::GitError> = local_stash_apply;
    fn switch_with_autostash(target: &git_engine::CheckoutTarget, message: &str, drop_after_clean: bool) -> Result<git_engine::AutostashOutcome, git_engine::GitError> = local_switch_with_autostash;
    fn stash_drop(oid: &str) -> Result<(), git_engine::GitError> = local_stash_drop;
    fn worktrees() -> Result<Vec<git_engine::WorktreeEntry>, git_engine::GitError> = local_worktrees;
    fn worktree_holding(branch: &str) -> Result<Option<git_engine::WorktreeEntry>, git_engine::GitError> = local_worktree_holding;
    fn add_worktree(path: &str, branch: &git_engine::WorktreeBranch) -> Result<(), git_engine::GitError> = local_add_worktree;
    fn check_revision(rev: &str) -> Result<git_engine::RevisionCheck, git_engine::GitError> = local_check_revision;
    fn check_branch_name(name: &str) -> Result<Option<String>, git_engine::GitError> = local_check_branch_name;
    fn remove_worktree(path: &str, force: bool) -> Result<(), git_engine::GitError> = local_remove_worktree;
    fn worktree_leftover(path: &str) -> Result<bool, git_engine::GitError> = local_worktree_leftover;
    fn delete_worktree_leftover(path: &str) -> Result<(), git_engine::GitError> = local_delete_worktree_leftover;
    fn prune_worktrees() -> Result<(), git_engine::GitError> = local_prune_worktrees;
    fn prune_worktree(path: &str) -> Result<(), git_engine::GitError> = local_prune_worktree;
    fn repair_worktree(path: &str) -> Result<(), git_engine::GitError> = local_repair_worktree;
    fn lock_worktree(path: &str, reason: Option<&str>) -> Result<(), git_engine::GitError> = local_lock_worktree;
    fn move_worktree(path: &str, to: &str) -> Result<(), git_engine::GitError> = local_move_worktree;
    fn unlock_worktree(path: &str) -> Result<(), git_engine::GitError> = local_unlock_worktree;
}
