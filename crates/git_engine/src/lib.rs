mod blobs;
mod branches;
mod commit;
mod commit_write;
mod error;
mod history;
mod repo;
mod runner;
mod search;
mod staging;
mod state;
mod status;
mod worktree;

pub use blobs::{DiffSides, DiffSpec};
pub use branches::CheckoutTarget;
pub use commit::{CommitDetails, FileEntry, FileStatus, Signature};
pub use commit_write::CommitRequest;
pub use error::{GitCommandError, GitError};
pub use history::CommitRow;
pub use repo::{Branch, BranchKind, Head, RepoHandle, Tag};
pub use runner::GitOutput;
pub use search::CommitQuery;
pub use state::RepoState;
pub use status::RepoStatus;
pub use worktree::WorktreeFiles;

pub type Result<T> = std::result::Result<T, GitError>;
