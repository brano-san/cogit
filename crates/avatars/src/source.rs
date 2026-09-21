//! The one place in the application that opens an HTTP connection (doc/12-risks.md, R-67).

use crate::identity::gravatar_url;
use crate::queue::{Fetched, Source};
use std::io::Read as _;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);
const SIZE: u32 = 64;
/// A 64×64 PNG is a few kilobytes; anything larger is not an avatar.
const MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug)]
pub struct Gravatar {
    agent: ureq::Agent,
}

impl Default for Gravatar {
    fn default() -> Self {
        Self::new()
    }
}

impl Gravatar {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(concat!("cogit/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Source for Gravatar {
    fn get(&self, email: &str) -> Fetched {
        match self.agent.get(&gravatar_url(email, SIZE)).call() {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let mut body = Vec::new();
                if response
                    .body_mut()
                    .as_reader()
                    .take(MAX_BYTES)
                    .read_to_end(&mut body)
                    .is_err()
                {
                    return Fetched::Failed;
                }
                outcome_for(status, body)
            }
            Err(ureq::Error::StatusCode(status)) => outcome_for(status, Vec::new()),
            // The address is never logged; only that a lookup failed.
            Err(error) => {
                tracing::debug!(?error, "an avatar lookup did not complete");
                Fetched::Failed
            }
        }
    }
}

/// Only a 404 means "no picture". Everything else that is not a body is a bad moment,
/// and remembering a bad moment as a miss would blank the author for seven days.
pub fn outcome_for(status: u16, body: Vec<u8>) -> Fetched {
    match status {
        200 if !body.is_empty() => Fetched::Image(body),
        200 | 404 => Fetched::Missing,
        _ => Fetched::Failed,
    }
}
