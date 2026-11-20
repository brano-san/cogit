use crate::sse::{Piece, SseReader, piece};
use crate::transport::CancelConnector;
use crate::{AiConfig, AiError, AiProvider, Prompt, provider};
use std::io::{BufRead, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;
use zeroize::Zeroizing;

use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{Connector, DefaultConnector};

const CONNECT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub text: String,
    pub stop_reason: Option<String>,
}

#[must_use]
pub fn redact_key(text: &str, key: Option<&str>) -> String {
    match key.map(str::trim).filter(|key| !key.is_empty()) {
        Some(key) => {
            let escaped = serde_json::Value::String(key.to_owned()).to_string();
            let encoded = &escaped[1..escaped.len() - 1];
            if encoded == key {
                text.replace(key, "<redacted>")
            } else {
                text.replace(encoded, "<redacted>")
                    .replace(key, "<redacted>")
            }
        }
        None => text.to_owned(),
    }
}

enum Message {
    Text(String),
    End(Result<Outcome, AiError>),
}

pub fn generate(
    config: &AiConfig,
    key: Option<&str>,
    prompt: &Prompt,
    proxy: Option<&str>,
    stopped: &dyn Fn() -> bool,
    on_text: &mut dyn FnMut(&str),
) -> Result<Outcome, AiError> {
    send_prompt(config, key, prompt, proxy, stopped, on_text, false)
}

pub fn test_provider(
    config: &AiConfig,
    key: Option<&str>,
    proxy: Option<&str>,
) -> Result<(), AiError> {
    let prompt = Prompt {
        system: "You check that a connection works.".into(),
        user: "Reply with the single word OK.".into(),
        shortened: false,
    };
    send_prompt(config, key, &prompt, proxy, &|| false, &mut |_| {}, true)?;
    Ok(())
}

fn send_prompt(
    config: &AiConfig,
    key: Option<&str>,
    prompt: &Prompt,
    proxy: Option<&str>,
    stopped: &dyn Fn() -> bool,
    on_text: &mut dyn FnMut(&str),
    test: bool,
) -> Result<Outcome, AiError> {
    if stopped() {
        return Err(AiError::Cancelled);
    }
    let endpoint = config.endpoint()?;
    let host = config.host()?;
    let key = key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| Zeroizing::new(key.to_owned()));
    if key.is_none() && config.needs_key()? {
        return Err(AiError::NoKey(config.provider.label().to_owned(), host));
    }
    if key.is_some() && config.is_plain_http() && !hosting::is_loopback(&host) {
        return Err(AiError::PlainHttp(host));
    }
    let body = provider::request_body(config, &prompt.system, &prompt.user, test).to_string();
    let http = hosting::http_config(&endpoint, proxy, None, Some(CONNECT)).map_err(|err| {
        AiError::Network(
            host.clone(),
            redact_key(&err.to_string(), key.as_deref().map(String::as_str)),
        )
    })?;
    let provider = config.provider;
    let quit = Arc::new(AtomicBool::new(false));
    let agent = ureq::Agent::with_parts(
        http,
        DefaultConnector::default().chain(CancelConnector(Arc::clone(&quit))),
        DefaultResolver::default(),
    );
    let (send, receive) = mpsc::channel();
    let worker = {
        let quit = Arc::clone(&quit);
        let host = host.clone();
        std::thread::Builder::new()
            .name("ai-message".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let mut held = String::new();
                let key = key.as_deref().map(String::as_str);
                let result = run(&mut Request {
                    agent: &agent,
                    endpoint: &endpoint,
                    host: &host,
                    provider,
                    key,
                    body: &body,
                    quit: &quit,
                    on_text: &mut |text| {
                        held.push_str(text);
                        let end = match key {
                            Some(key) => {
                                if held.contains(key) {
                                    held = held.replace(key, "<redacted>");
                                }
                                held.char_indices()
                                    .find(|(index, _)| key.starts_with(&held[*index..]))
                                    .map_or(held.len(), |(index, _)| index)
                            }
                            None => held.len(),
                        };
                        if end > 0 {
                            let _ = send.send(Message::Text(held.drain(..end).collect()));
                        }
                    },
                });
                tracing::info!(%host, elapsed = ?started.elapsed(), ok = result.is_ok(), "AI commit message request");
                let result = result.map(|mut outcome| {
                    if !held.is_empty() {
                        let _ = send.send(Message::Text(held));
                    }
                    outcome.text = redact_key(&outcome.text, key);
                    outcome
                }).map_err(|err| err.redacted(key));
                let _ = send.send(Message::End(result));
            })
            .map_err(|err| AiError::Network(String::new(), err.to_string()))?
    };
    loop {
        if stopped() {
            quit.store(true, Ordering::Relaxed);
            return Err(AiError::Cancelled);
        }
        match receive.recv_timeout(POLL) {
            Ok(Message::Text(text)) => on_text(&text),
            Ok(Message::End(result)) => {
                let _ = worker.join();
                return result;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AiError::Reply(
                    host,
                    "the request ended without an answer".into(),
                ));
            }
        }
    }
}

struct Request<'a> {
    agent: &'a ureq::Agent,
    endpoint: &'a str,
    host: &'a str,
    provider: AiProvider,
    key: Option<&'a str>,
    body: &'a str,
    quit: &'a AtomicBool,
    on_text: &'a mut dyn FnMut(&str),
}

fn run(request: &mut Request<'_>) -> Result<Outcome, AiError> {
    let host = request.host.to_owned();
    let mut call = request
        .agent
        .post(request.endpoint)
        .config()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .max_idle_connections(0)
        .build();
    for (name, value) in provider::headers(request.provider, request.key) {
        call = call.header(name, value);
    }
    let mut response = call
        .send(request.body)
        .map_err(|err| AiError::Network(host.clone(), err.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let mut bytes = Vec::new();
        let failure = response
            .body_mut()
            .as_reader()
            .read_to_end(&mut bytes)
            .err();
        let mut body = String::from_utf8_lossy(&bytes).into_owned();
        if let Some(err) = failure {
            use std::fmt::Write as _;
            let _ = write!(body, "\nCould not finish reading the response: {err}");
        }
        return Err(AiError::Status(host, status, body));
    }
    let reader = std::io::BufReader::new(response.into_body().into_reader());
    read_stream(
        reader,
        request.provider,
        &host,
        request.quit,
        request.on_text,
    )
}

pub fn read_stream(
    mut reader: impl BufRead,
    provider: AiProvider,
    host: &str,
    quit: &AtomicBool,
    on_text: &mut dyn FnMut(&str),
) -> Result<Outcome, AiError> {
    let mut events = SseReader::default();
    let mut text = String::new();
    let mut stop_reason = None;
    let mut done = false;
    let mut line = String::new();
    loop {
        if quit.load(Ordering::Relaxed) {
            return Err(AiError::Cancelled);
        }
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| AiError::Network(host.to_owned(), err.to_string()))?;
        let event = if read == 0 {
            events.finish()
        } else {
            events.line(&line)
        };
        if let Some(event) = event {
            match piece(provider, &event) {
                Piece::Text(delta) => {
                    on_text(&delta);
                    text.push_str(&delta);
                }
                Piece::Stop(reason) => stop_reason = Some(reason),
                Piece::Done => done = true,
                Piece::Failed(raw) => return Err(AiError::Stream(host.to_owned(), raw)),
                Piece::Nothing => {}
            }
        }
        if done || read == 0 {
            break;
        }
    }
    if !done && stop_reason.is_none() {
        return Err(AiError::Reply(
            host.to_owned(),
            "the answer stopped before it was complete".into(),
        ));
    }
    if text.trim().is_empty() {
        return Err(AiError::Empty(
            host.to_owned(),
            stop_reason.unwrap_or_else(|| "none".into()),
        ));
    }
    Ok(Outcome { text, stop_reason })
}
