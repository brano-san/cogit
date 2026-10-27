mod client;
mod frame;
mod server;

pub use client::{AgentError, Client};
pub use frame::{FromAgent, PROTOCOL, ToAgent, read_frame, write_frame};
pub use server::{Call, Events, serve};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
