mod client;
mod prompt;
mod provider;
mod sse;
mod transport;

pub use client::{Outcome, generate, read_stream, redact_key, test_provider};
pub use prompt::{
    Change, DIFF_BUDGET, Prompt, Skipped, Staged, build, build_with_budget, clean_message, shorten,
};
pub use provider::{ANTHROPIC_VERSION, AiConfig, AiProvider, headers, request_body, takes_effort};
pub use sse::{Piece, SseEvent, SseReader, piece};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    #[error("Enter an HTTP or HTTPS base URL without credentials, a query, or a fragment.")]
    BaseUrl,
    #[error("Add an API key for {0} at {1} in Preferences ▸ AI.")]
    NoKey(String, String),
    #[error("Cogit sends an API key over plain http:// only to this machine; use https:// for {0}")]
    PlainHttp(String),
    #[error("{0} answered HTTP {1}: {2}")]
    Status(String, u16, String),
    #[error("could not reach {0}: {1}")]
    Network(String, String),
    #[error("{0} reported an error: {1}")]
    Stream(String, String),
    #[error("{0} sent an answer Cogit does not understand: {1}")]
    Reply(String, String),
    #[error("{0} returned no message (stop reason: {1})")]
    Empty(String, String),
    #[error("canceled")]
    Cancelled,
}

impl AiError {
    #[must_use]
    pub fn redacted(self, key: Option<&str>) -> Self {
        let clean = |text: String| redact_key(&text, key);
        match self {
            Self::Status(host, status, body) => Self::Status(host, status, clean(body)),
            Self::Network(host, text) => Self::Network(host, clean(text)),
            Self::Stream(host, text) => Self::Stream(host, clean(text)),
            Self::Reply(host, text) => Self::Reply(host, clean(text)),
            Self::Empty(host, reason) => Self::Empty(host, clean(reason)),
            other => other,
        }
    }
}
