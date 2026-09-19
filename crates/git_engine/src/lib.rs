//! Git reading (via `gix`) and mutations (via the system `git` CLI).
//!
//! This crate is the only place in Cogit that knows how Git is laid out.
//! See `doc/03-git-semantics.md` for the per-operation split between `gix` and the CLI.
//!
//! Module layout to be filled in during M1:
//! - `repo`     — `RepoManager`, `RepoHandle`, discovery and reads through `gix`
//! - `cli`      — the single entry point for spawning `git`
//! - `status`   — working tree / index / HEAD comparison
//! - `refs`     — branches, tags, remotes, upstream relationships
//! - `history`  — commit iteration for the graph
//! - `submodule`— `.gitmodules` parsing

mod error;
mod state;

pub use error::{GitCommandError, GitError};
pub use state::RepoState;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, GitError>;
