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

enum Message {
    Chunk(Value),
    Done(Outcome),
}

#[derive(Default)]
struct Waiting {
    calls: HashMap<u64, Sender<Message>>,
    gone: Option<String>,
}

type Output = Arc<Mutex<Box<dyn Write + Send>>>;

pub struct Client {
    output: Output,
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
    pub fn connect<R, W>(input: R, output: W) -> Result<Self, AgentError>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        Self::connect_with_events(input, output, |_| {})
    }

    pub fn connect_with_events<R, W, E>(
        input: R,
        output: W,
        on_event: E,
    ) -> Result<Self, AgentError>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
        E: Fn(Value) + Send + 'static,
    {
        Self::connect_with(input, output, on_event, |_| Value::Null)
    }

    pub fn connect_with<R, W, E, A>(
        mut input: R,
        mut output: W,
        on_event: E,
        on_ask: A,
    ) -> Result<Self, AgentError>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
        E: Fn(Value) + Send + 'static,
        A: Fn(Value) -> Value + Send + Sync + 'static,
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
        let output: Output = Arc::new(Mutex::new(Box::new(output)));
        let answers = output.clone();
        let on_ask = Arc::new(on_ask);
        std::thread::Builder::new()
            .name("agent-replies".into())
            .spawn(move || {
                deliver_replies(&mut input, &replies, &on_event, &|id, question| {
                    let (answers, on_ask) = (answers.clone(), on_ask.clone());
                    std::thread::spawn(move || {
                        let answer = ToAgent::Answer {
                            id,
                            answer: on_ask(question),
                        };
                        if let Err(err) = write_frame(&mut *answers.lock(), &answer) {
                            tracing::error!(error = ?err, id, context = "answer to the agent");
                        }
                    });
                });
            })
            .map_err(|err| AgentError::Handshake(err.to_string()))?;
        Ok(Self {
            output,
            waiting,
            next: AtomicU64::new(1),
            version,
        })
    }

    #[must_use]
    pub fn is_gone(&self) -> bool {
        self.waiting.lock().gone.is_some()
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn call(&self, request: Value, stop: Option<&NetworkStop>) -> Result<Outcome, AgentError> {
        self.call_streaming(
            request,
            &|| stop.is_some_and(NetworkStop::is_stopped),
            &mut |_| {},
        )
    }

    pub fn call_streaming(
        &self,
        request: Value,
        canceled: &dyn Fn() -> bool,
        on_chunk: &mut dyn FnMut(Value),
    ) -> Result<Outcome, AgentError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        {
            let mut waiting = self.waiting.lock();
            if let Some(reason) = &waiting.gone {
                return Err(AgentError::Gone(reason.clone()));
            }
            waiting.calls.insert(id, reply);
        }
        let mut asked_to_stop = canceled();
        self.send(&ToAgent::Call {
            id,
            request,
            stopped: asked_to_stop,
        })?;
        loop {
            match answer.recv_timeout(STOP_POLL) {
                Ok(Message::Done(outcome)) => return Ok(outcome),
                Ok(Message::Chunk(data)) => on_chunk(data),
                Err(RecvTimeoutError::Disconnected) => return Err(self.gone()),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if !asked_to_stop && canceled() {
                asked_to_stop = true;
                self.send(&ToAgent::Cancel { id })?;
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

fn deliver_replies<R: Read>(
    input: &mut R,
    waiting: &Mutex<Waiting>,
    on_event: &dyn Fn(Value),
    on_ask: &dyn Fn(u64, Value),
) {
    let reason = loop {
        match read_frame(input) {
            Ok(Some(FromAgent::Reply { id, outcome })) => {
                if let Some(reply) = waiting.lock().calls.remove(&id) {
                    reply.send(Message::Done(outcome)).ok();
                }
            }
            Ok(Some(FromAgent::Chunk { id, data })) => {
                if let Some(reply) = waiting.lock().calls.get(&id) {
                    reply.send(Message::Chunk(data)).ok();
                }
            }
            Ok(Some(FromAgent::Event { event })) => on_event(event),
            Ok(Some(FromAgent::Ask { id, question })) => on_ask(id, question),
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
