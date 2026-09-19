mod blobs;
mod commit;
mod error;
mod history;
mod repo;
mod state;
mod status;

pub use blobs::{DiffSides, DiffSpec};
pub use commit::{CommitDetails, FileEntry, FileStatus, Signature};
pub use error::{GitCommandError, GitError};
pub use history::CommitRow;
pub use repo::{Branch, BranchKind, Head, RepoHandle, Tag};
pub use state::RepoState;
pub use status::RepoStatus;

pub type Result<T> = std::result::Result<T, GitError>;
