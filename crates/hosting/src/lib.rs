mod account;
mod api;
mod ci;
mod ci_cache;
mod remote;

pub use account::{AccountError, HostingAccount, HostingProvider, normalize_server};
pub use api::{ApiError, Auth, Limit, Reply, auth_for, check, check_at, get_json, get_reply};
pub use ci::{
    CiCheck, CiReport, CiState, CiStatus, aggregate, bitbucket_statuses, commit_page, fetch_checks,
    github_check_runs, github_statuses, gitlab_statuses,
};
pub use ci_cache::{CiCache, Lookup, ttl};
pub use remote::{HostingMatch, RemoteRef, account_for};
