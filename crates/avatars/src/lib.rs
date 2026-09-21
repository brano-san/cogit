//! Author avatars: a Gravatar lookup, an on-disk cache and a local fallback (M14).

mod cache;
mod fallback;
mod identity;
mod queue;
mod source;

pub use cache::{Cache, CacheError, Lookup};
pub use fallback::{Fallback, fallback};
pub use identity::{email_hash, gravatar_url, is_noreply};
pub use queue::{Fetched, Queue, Source};
pub use source::{Gravatar, outcome_for};
