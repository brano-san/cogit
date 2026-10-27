use crate::frame::{FromAgent, PROTOCOL, ToAgent, read_frame, write_frame};
use git_engine::NetworkStop;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

const STOP_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("the agent speaks protocol {agent} (version {version}), Cogit speaks {ours}")]
    Protocol {
        ours: u32,
        agent: u32,
        version: String,
    },
    #[error("the agent did not answer the handshake: {0}")]
    Handshake(String),
    #[error("the connection to the agent is lost: {0}")]
    Gone(String),
}

type Outcome = Result<Value, Value>;

#[derive(Default)]
struct Waiting {
    calls: HashMap<u64, Sender<Outcome>>,
    gone: Option<String>,
}

pub struct Client {
    output: Mutex<Box<dyn Write + Send>>,
    waiting: Arc<Mutex<Waiting>>,
    next: AtomicU64,
    version: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub fn connect<R, W>(mut input: R, mut output: W) -> Result<Self, AgentError>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let hello = ToAgent::Hello {
            protocol: PROTOCOL,
            version: crate::VERSION.to_owned(),
        };
        write_frame(&mut output, &hello).map_err(|err| AgentError::Handshake(err.to_string()))?;
        let version = match read_frame(&mut input) {
            Ok(Some(FromAgent::Welcome { protocol, version })) if protocol == PROTOCOL => version,
            Ok(Some(FromAgent::Welcome { protocol, version })) => {
                return Err(AgentError::Protocol {
                    ours: PROTOCOL,
                    agent: protocol,
                    version,
                });
            }
            Ok(Some(other)) => return Err(AgentError::Handshake(format!("{other:?}"))),
            Ok(None) => return Err(AgentError::Handshake("the agent closed its output".into())),
            Err(err) => return Err(AgentError::Handshake(err.to_string())),
        };
        tracing::info!(version, "agent connected");
        let waiting = Arc::new(Mutex::new(Waiting::default()));
        let replies = waiting.clone();
        std::thread::Builder::new()
            .name("agent-replies".into())
            .spawn(move || deliver_replies(&mut input, &replies))
            .map_err(|err| AgentError::Handshake(err.to_string()))?;
        Ok(Self {
            output: Mutex::new(Box::new(output)),
            waiting,
            next: AtomicU64::new(1),
            version,
        })
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn call(&self, request: Value, stop: Option<&NetworkStop>) -> Result<Outcome, AgentError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        {
            let mut waiting = self.waiting.lock();
            if let Some(reason) = &waiting.gone {
                return Err(AgentError::Gone(reason.clone()));
            }
            waiting.calls.insert(id, reply);
        }
        self.send(&ToAgent::Call { id, request })?;
        let mut canceled = false;
        loop {
            match answer.recv_timeout(STOP_POLL) {
                Ok(outcome) => return Ok(outcome),
                Err(RecvTimeoutError::Disconnected) => return Err(self.gone()),
                Err(RecvTimeoutError::Timeout) => {
                    if !canceled && stop.is_some_and(NetworkStop::is_stopped) {
                        canceled = true;
                        self.send(&ToAgent::Cancel { id })?;
                    }
                }
            }
        }
    }

    fn send(&self, frame: &ToAgent) -> Result<(), AgentError> {
        write_frame(&mut *self.output.lock(), frame).map_err(|err| {
            let reason = err.to_string();
            fail_all(&self.waiting, &reason);
            AgentError::Gone(reason)
        })
    }

    fn gone(&self) -> AgentError {
        let reason = self.waiting.lock().gone.clone();
        AgentError::Gone(reason.unwrap_or_else(|| "no reply".to_owned()))
    }
}

fn deliver_replies<R: Read>(input: &mut R, waiting: &Mutex<Waiting>) {
    let reason = loop {
        match read_frame(input) {
            Ok(Some(FromAgent::Reply { id, outcome })) => {
                if let Some(reply) = waiting.lock().calls.remove(&id) {
                    reply.send(outcome).ok();
                }
            }
            Ok(Some(other)) => break format!("unexpected frame {other:?}"),
            Ok(None) => break "the agent closed its output".to_owned(),
            Err(err) => break err.to_string(),
        }
    };
    tracing::error!(reason, "agent connection lost");
    fail_all(waiting, &reason);
}

fn fail_all(waiting: &Mutex<Waiting>, reason: &str) {
    let mut waiting = waiting.lock();
    waiting.gone.get_or_insert_with(|| reason.to_owned());
    waiting.calls.clear();
}
