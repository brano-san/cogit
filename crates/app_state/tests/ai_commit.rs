#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AiConfig, AiKeyInput, AiMessageRequest, AiProvider, AppState, SignIn, StoreState};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

const KEY: &str = "sk-ant-api03-very-secret-key";

fn state_with_store() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));
    state.create_credential_store(None).unwrap();
    (dir, state)
}

fn config(provider: AiProvider, base_url: &str) -> AiConfig {
    AiConfig {
        provider,
        base_url: base_url.to_owned(),
        model: String::new(),
        instructions: String::new(),
        max_diff_bytes: 40_000,
    }
}

fn input(config: AiConfig, key: &str) -> AiKeyInput {
    AiKeyInput {
        config,
        key: key.to_owned(),
    }
}

fn serve(status: u16, body: String) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (send, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap();
            }
        }
        let mut request = vec![0; length];
        reader.read_exact(&mut request).unwrap();
        send.send(String::from_utf8(request).unwrap()).unwrap();
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (base, seen)
}

#[test]
fn a_saved_key_stays_out_of_the_sign_ins_the_listing_and_the_file() {
    let (dir, state) = state_with_store();
    let anthropic = config(AiProvider::Anthropic, "");
    assert!(!state.ai_key_state(&anthropic).unwrap().stored);

    state
        .set_ai_key(&input(anthropic.clone(), &format!(" {KEY} ")))
        .unwrap();

    let key_state = state.ai_key_state(&anthropic).unwrap();
    assert_eq!(key_state.state, StoreState::Unlocked);
    assert_eq!(key_state.host, "api.anthropic.com");
    assert!(key_state.stored);
    assert!(!serde_json::to_string(&key_state).unwrap().contains(KEY));
    assert!(state.credential_store().unwrap().known.is_empty());
    assert!(
        state
            .stored_credential("https://api.anthropic.com/x.git")
            .is_none()
    );
    assert!(
        !state
            .ai_key_state(&config(AiProvider::OpenAi, "https://api.anthropic.com"))
            .unwrap()
            .stored
    );
    assert!(
        !state
            .ai_key_state(&config(AiProvider::Anthropic, "https://gateway.example"))
            .unwrap()
            .stored
    );
    let file = std::fs::read_to_string(dir.path().join("passwords.json")).unwrap();
    assert!(!file.contains(KEY));
    assert!(!format!("{:?}", input(anthropic.clone(), KEY)).contains(KEY));

    state.forget_all_credentials().unwrap();
    assert!(state.ai_key_state(&anthropic).unwrap().stored);
    state.remove_ai_key(&anthropic).unwrap();
    assert!(!state.ai_key_state(&anthropic).unwrap().stored);
}

#[test]
fn a_sign_in_for_the_same_host_is_a_different_entry() {
    let (_dir, state) = state_with_store();
    let openai = config(AiProvider::OpenAi, "https://api.openai.com/v1");
    state
        .set_ai_key(&input(openai.clone(), "sk-openai"))
        .unwrap();
    state
        .store_credential(&SignIn {
            host: "api.openai.com".into(),
            username: Some("ann".into()),
            secret: "pw".into(),
            key_path: None,
            remember: true,
        })
        .unwrap();
    state.forget_credential("api.openai.com", false).unwrap();
    assert!(state.ai_key_state(&openai).unwrap().stored);
}

#[test]
fn a_locked_store_makes_no_provider_request() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("x.txt", "x\n").unwrap();
    f.git(&["add", "x.txt"]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("passwords.json");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = config(
        AiProvider::OpenAi,
        &format!("http://{}", listener.local_addr().unwrap()),
    );
    let state = AppState::new();
    state.use_password_store(path.clone());
    assert_eq!(
        state.ai_key_state(&config).unwrap().state,
        StoreState::Missing
    );
    state.create_credential_store(Some("passphrase")).unwrap();
    state.set_ai_key(&input(config.clone(), KEY)).unwrap();
    state.use_password_store(path);
    assert_eq!(
        state.ai_key_state(&config).unwrap().state,
        StoreState::Locked
    );
    let repo = state.open_repository(f.path()).unwrap().repo;
    let request = AiMessageRequest {
        config,
        prefix: None,
    };
    let error = state
        .generate_commit_message(repo, &request, &|| false, &mut |_| {})
        .unwrap_err();
    assert!(error.to_string().to_lowercase().contains("locked"));
    assert!(state.test_ai_provider(&request.config).is_err());
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn a_generated_message_comes_from_the_staged_diff_and_streams() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("feature.txt", "brand new line\n").unwrap();
    f.git(&["add", "feature.txt"]).unwrap();
    let (_dir, state) = state_with_store();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let answer = [
        r#"data: {"choices":[{"index":0,"delta":{"content":"```\nABC-1: add "},"finish_reason":null}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"content":"feature\n```"},"finish_reason":"stop"}]}"#,
        "data: [DONE]",
    ]
    .map(|line| format!("{line}\n\n"))
    .concat();
    let (base, seen) = serve(200, answer);
    let request = AiMessageRequest {
        config: AiConfig {
            instructions: "Mention the ticket.".into(),
            ..config(AiProvider::OpenAi, &base)
        },
        prefix: Some("ABC-1: ".into()),
    };
    let mut streamed = String::new();
    let made = state
        .generate_commit_message(repo, &request, &|| false, &mut |text| {
            streamed.push_str(text)
        })
        .unwrap();
    assert_eq!(made.message, "ABC-1: add feature");
    assert!(!made.shortened);
    assert!(streamed.contains("add feature"));
    let sent = seen.recv().unwrap();
    assert!(sent.contains("+brand new line"));
    assert!(sent.contains("Mention the ticket."));
    assert!(sent.contains("Begin the first line with exactly this text: ABC-1: "));
}

#[test]
fn nothing_staged_sends_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    let (_dir, state) = state_with_store();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let request = AiMessageRequest {
        config: config(AiProvider::OpenAi, "http://127.0.0.1:9/v1"),
        prefix: None,
    };
    let err = state
        .generate_commit_message(repo, &request, &|| false, &mut |_| {})
        .unwrap_err();
    assert!(err.to_string().contains("nothing is staged"));
}

#[test]
fn a_provider_error_is_raw_and_without_the_key() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("x.txt", "x\n").unwrap();
    f.git(&["add", "x.txt"]).unwrap();
    let (_dir, state) = state_with_store();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let (base, _seen) = serve(
        401,
        format!(
            r#"{{"error":{{"message":"Incorrect API key provided: {KEY}","type":"invalid_request_error"}}}}"#
        ),
    );
    let local = config(AiProvider::OpenAi, &base);
    state.set_ai_key(&input(local.clone(), KEY)).unwrap();
    let err = state
        .generate_commit_message(
            repo,
            &AiMessageRequest {
                config: local,
                prefix: None,
            },
            &|| false,
            &mut |_| {},
        )
        .unwrap_err();
    let text = err.to_string();
    assert!(text.contains("HTTP 401"), "{text}");
    assert!(
        text.contains("Incorrect API key provided: <redacted>"),
        "{text}"
    );
    assert!(!text.contains(KEY));
    let as_git: git_engine::GitError = err.into();
    assert!(!serde_json::to_string(&as_git).unwrap().contains(KEY));
}
