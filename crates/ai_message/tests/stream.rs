#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_message::{
    AiConfig, AiError, AiProvider, Change, Prompt, Staged, build, generate, read_stream,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const KEY: &str = "sk-test-0123456789-secret";

fn sample(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/samples")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn read(provider: AiProvider, text: &str) -> (Result<ai_message::Outcome, AiError>, String) {
    let mut streamed = String::new();
    let result = read_stream(
        BufReader::new(text.as_bytes()),
        provider,
        "api.example",
        &AtomicBool::new(false),
        &mut |delta| streamed.push_str(delta),
    );
    (result, streamed)
}

#[test]
fn an_anthropic_stream_gives_the_text_and_skips_thinking() {
    let (result, streamed) = read(AiProvider::Anthropic, &sample("anthropic.sse"));
    let outcome = result.unwrap();
    assert_eq!(
        outcome.text,
        "feat(ui): add a Generate Message button\n\nAsks the model for a message."
    );
    assert_eq!(streamed, outcome.text);
    assert_eq!(outcome.stop_reason.as_deref(), Some("end_turn"));
}

#[test]
fn an_anthropic_error_event_is_shown_raw() {
    let (result, _) = read(AiProvider::Anthropic, &sample("anthropic-error.sse"));
    let err = result.unwrap_err();
    assert!(matches!(err, AiError::Stream(..)));
    assert!(err.to_string().contains(r#""type": "overloaded_error""#));
}

#[test]
fn an_openai_stream_gives_the_text() {
    let (result, _) = read(AiProvider::OpenAi, &sample("openai.sse"));
    let outcome = result.unwrap();
    assert_eq!(outcome.text, "Fix the crash on start");
    assert_eq!(outcome.stop_reason.as_deref(), Some("stop"));
}

#[test]
fn an_openai_error_chunk_is_shown_raw() {
    let (result, _) = read(AiProvider::OpenAi, &sample("openai-error.sse"));
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("The server had an error while processing your request.")
    );
}

#[test]
fn a_stream_cut_short_is_an_error_not_a_message() {
    let text = sample("anthropic.sse");
    let cut = &text[..text
        .find("event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}")
        .unwrap()];
    let (result, streamed) = read(AiProvider::Anthropic, cut);
    assert!(matches!(result, Err(AiError::Reply(..))));
    assert!(!streamed.is_empty());
}

struct Seen {
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

fn serve(
    status: u16,
    content_type: &'static str,
    parts: Vec<String>,
    pause: Duration,
) -> (String, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (send, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
        let mut headers = Vec::new();
        let mut length = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                let (name, value) = (name.trim().to_lowercase(), value.trim().to_owned());
                if name == "content-length" {
                    length = value.parse().unwrap();
                }
                headers.push((name, value));
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        send.send(Seen {
            path,
            headers,
            body: String::from_utf8(body).unwrap(),
        })
        .unwrap();
        let mut stream = stream;
        let _ = write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n"
        );
        for part in parts {
            if stream.write_all(part.as_bytes()).is_err() {
                return;
            }
            let _ = stream.flush();
            std::thread::sleep(pause);
        }
    });
    (base, seen)
}

fn config(provider: AiProvider, base: &str) -> AiConfig {
    AiConfig {
        provider,
        base_url: base.to_owned(),
        model: String::new(),
        instructions: String::new(),
        max_diff_bytes: 40_000,
    }
}

fn prompt() -> Prompt {
    build(
        &Staged {
            branch: Some("main".into()),
            subjects: vec!["fix: a".into()],
            changes: vec![Change {
                path: "a.rs".into(),
                old_path: None,
                added: Some(1),
                deleted: Some(0),
                skipped: None,
            }],
            diff: "diff --git a/a.rs b/a.rs\n@@ -0,0 +1 @@\n+fn main() {}\n".into(),
        },
        None,
        "",
    )
}

fn header<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
    seen.headers
        .iter()
        .find(|(each, _)| each == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn anthropic_end_to_end_streams_the_message() {
    let events: Vec<String> = sample("anthropic.sse")
        .split_inclusive("\n\n")
        .map(str::to_owned)
        .collect();
    let (base, seen) = serve(200, "text/event-stream", events, Duration::from_millis(5));
    let mut pieces = Vec::new();
    let outcome = generate(
        &config(AiProvider::Anthropic, &base),
        Some(KEY),
        &prompt(),
        None,
        &|| false,
        &mut |text| pieces.push(text.to_owned()),
    )
    .unwrap();
    assert_eq!(pieces.len(), 2);
    assert!(outcome.text.starts_with("feat(ui): add a Generate"));
    let seen = seen.recv().unwrap();
    assert_eq!(seen.path, "/v1/messages");
    assert_eq!(header(&seen, "x-api-key"), Some(KEY));
    assert_eq!(header(&seen, "anthropic-version"), Some("2023-06-01"));
    assert_eq!(header(&seen, "authorization"), None);
    let body: serde_json::Value = serde_json::from_str(&seen.body).unwrap();
    assert_eq!(body["model"], "claude-haiku-5-5");
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("+fn main() {}")
    );
    assert!(!seen.body.contains(KEY));
}

#[test]
fn openai_end_to_end_sends_a_bearer_key() {
    let (base, seen) = serve(
        200,
        "text/event-stream",
        vec![sample("openai.sse")],
        Duration::ZERO,
    );
    let outcome = generate(
        &config(AiProvider::OpenAi, &format!("{base}/v1")),
        Some(KEY),
        &prompt(),
        None,
        &|| false,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(outcome.text, "Fix the crash on start");
    let seen = seen.recv().unwrap();
    assert_eq!(seen.path, "/v1/chat/completions");
    assert_eq!(
        header(&seen, "authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
}

#[test]
fn a_local_server_needs_no_key() {
    let (base, seen) = serve(
        200,
        "text/event-stream",
        vec![sample("openai.sse")],
        Duration::ZERO,
    );
    generate(
        &config(AiProvider::OpenAi, &base),
        None,
        &prompt(),
        None,
        &|| false,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(header(&seen.recv().unwrap(), "authorization"), None);
}

#[test]
fn a_refusal_shows_the_providers_body_without_the_key() {
    let body = format!(
        r#"{{"type":"error","error":{{"type":"authentication_error","message":"invalid x-api-key {KEY}"}}}}"#
    );
    let (base, _seen) = serve(401, "application/json", vec![body], Duration::ZERO);
    let err = generate(
        &config(AiProvider::Anthropic, &base),
        Some(KEY),
        &prompt(),
        None,
        &|| false,
        &mut |_| {},
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(matches!(err, AiError::Status(_, 401, _)));
    assert!(text.contains("authentication_error"));
    assert!(text.contains("invalid x-api-key <redacted>"));
    assert!(!text.contains(KEY));
    assert!(!format!("{err:?}").contains(KEY));
}

#[test]
fn anthropic_without_a_key_is_refused_before_sending() {
    let err = generate(
        &config(AiProvider::Anthropic, "https://api.anthropic.com"),
        None,
        &prompt(),
        None,
        &|| false,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, AiError::NoKey(..)));
}

#[test]
fn a_key_never_goes_over_plain_http_to_another_machine() {
    let err = generate(
        &config(AiProvider::OpenAi, "http://ollama.lan:11434/v1"),
        Some(KEY),
        &prompt(),
        None,
        &|| false,
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err, AiError::PlainHttp("ollama.lan".into()));
}

#[test]
fn cancel_closes_the_connection_while_the_server_is_idle() {
    let first = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"fix\"}}\n\n".to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            request.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap();
            }
        }
        request.read_exact(&mut vec![0; length]).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{first}"
        )
        .unwrap();
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let got_text = AtomicBool::new(false);
    let started = Instant::now();
    let err = generate(
        &config(AiProvider::Anthropic, &base),
        Some(KEY),
        &prompt(),
        None,
        &|| got_text.load(Ordering::Relaxed) && started.elapsed() > Duration::from_millis(300),
        &mut |_| got_text.store(true, Ordering::Relaxed),
    )
    .unwrap_err();
    assert_eq!(err, AiError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
    server.join().unwrap();
}
