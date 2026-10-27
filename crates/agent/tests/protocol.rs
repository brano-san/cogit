#![allow(clippy::unwrap_used)]

use agent::{AgentError, Client, FromAgent, PROTOCOL, ToAgent, read_frame, serve, write_frame};
use git_engine::NetworkStop;
use serde_json::{Value, json};
use std::io::{self, PipeReader, PipeWriter};
use std::sync::{Arc, Barrier, mpsc};
use std::thread::{self, JoinHandle};

type Handler = Box<dyn Fn(Value, &NetworkStop) -> Result<Value, Value> + Send + Sync>;

struct Wires {
    agent_in: PipeReader,
    agent_out: PipeWriter,
    client_in: PipeReader,
    client_out: PipeWriter,
}

fn wires() -> Wires {
    let (agent_in, client_out) = io::pipe().unwrap();
    let (client_in, agent_out) = io::pipe().unwrap();
    Wires {
        agent_in,
        agent_out,
        client_in,
        client_out,
    }
}

fn pair(handler: Handler) -> (Client, JoinHandle<io::Result<()>>) {
    let wires = wires();
    let server = thread::spawn(move || serve(wires.agent_in, wires.agent_out, handler));
    let client = Client::connect(wires.client_in, wires.client_out).unwrap();
    (client, server)
}

#[test]
fn the_handshake_names_the_agent_version() {
    let (client, _server) = pair(Box::new(|request, _| Ok(request)));
    assert_eq!(client.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn ten_parallel_calls_get_their_own_answers() {
    let all_in = Arc::new(Barrier::new(10));
    let (client, _server) = pair(Box::new(move |request, _| {
        all_in.wait();
        Ok(json!({ "echo": request }))
    }));
    let client = Arc::new(client);
    let callers: Vec<_> = (0..10)
        .map(|n| {
            let client = client.clone();
            thread::spawn(move || (n, client.call(json!(n), None)))
        })
        .collect();
    for caller in callers {
        let (n, answer) = caller.join().unwrap();
        assert_eq!(answer.unwrap(), Ok(json!({ "echo": n })));
    }
}

#[test]
fn an_error_of_the_handler_comes_back_as_an_error() {
    let (client, _server) = pair(Box::new(|_, _| Err(json!("no such repository"))));
    assert_eq!(
        client.call(json!("open"), None).unwrap(),
        Err(json!("no such repository"))
    );
}

#[test]
fn a_cancel_reaches_the_running_call() {
    let (started_tx, started) = mpsc::channel();
    let started_tx = parking_lot::Mutex::new(started_tx);
    let (client, _server) = pair(Box::new(move |_, stop| {
        started_tx.lock().send(()).unwrap();
        while !stop.is_stopped() {
            thread::yield_now();
        }
        Err(json!("canceled"))
    }));
    let stop = NetworkStop::default();
    let asker = stop.clone();
    thread::spawn(move || {
        started.recv().unwrap();
        asker.stop();
    });
    assert_eq!(
        client.call(json!("fetch"), Some(&stop)).unwrap(),
        Err(json!("canceled"))
    );
}

#[test]
fn closing_the_pipe_stops_the_running_calls_and_ends_the_agent() {
    let wires = wires();
    let (started_tx, started) = mpsc::channel();
    let started_tx = parking_lot::Mutex::new(started_tx);
    let server = thread::spawn(move || {
        serve(
            wires.agent_in,
            wires.agent_out,
            move |_, stop: &NetworkStop| {
                started_tx.lock().send(()).unwrap();
                while !stop.is_stopped() {
                    thread::yield_now();
                }
                Err(json!("canceled"))
            },
        )
    });
    let mut out = wires.client_out;
    write_frame(
        &mut out,
        &ToAgent::Hello {
            protocol: PROTOCOL,
            version: "test".into(),
        },
    )
    .unwrap();
    write_frame(
        &mut out,
        &ToAgent::Call {
            id: 1,
            request: json!("fetch"),
        },
    )
    .unwrap();
    started.recv().unwrap();
    drop(out);
    server.join().unwrap().unwrap();
    drop(wires.client_in);
}

#[test]
fn an_agent_that_dies_fails_the_waiting_call_and_every_later_one() {
    let wires = wires();
    let agent = thread::spawn(move || {
        let mut input = wires.agent_in;
        let mut output = wires.agent_out;
        let _: Option<ToAgent> = read_frame(&mut input).unwrap();
        write_frame(
            &mut output,
            &FromAgent::Welcome {
                protocol: PROTOCOL,
                version: "test".into(),
            },
        )
        .unwrap();
        let _: Option<ToAgent> = read_frame(&mut input).unwrap();
    });
    let client = Client::connect(wires.client_in, wires.client_out).unwrap();
    assert!(matches!(
        client.call(json!("status"), None),
        Err(AgentError::Gone(_))
    ));
    agent.join().unwrap();
    assert!(matches!(
        client.call(json!("status"), None),
        Err(AgentError::Gone(_))
    ));
}

#[test]
fn another_protocol_is_refused_at_the_handshake() {
    let wires = wires();
    thread::spawn(move || {
        let mut input = wires.agent_in;
        let mut output = wires.agent_out;
        let _: Option<ToAgent> = read_frame(&mut input).unwrap();
        write_frame(
            &mut output,
            &FromAgent::Welcome {
                protocol: PROTOCOL + 1,
                version: "old".into(),
            },
        )
        .unwrap();
    });
    let refused = Client::connect(wires.client_in, wires.client_out).err();
    assert!(matches!(
        refused,
        Some(AgentError::Protocol { agent, .. }) if agent == PROTOCOL + 1
    ));
}

#[test]
fn an_agent_hears_another_protocol_and_leaves_after_saying_its_own() {
    let wires = wires();
    let server = thread::spawn(move || serve(wires.agent_in, wires.agent_out, |r, _| Ok(r)));
    let mut out = wires.client_out;
    let mut input = wires.client_in;
    write_frame(
        &mut out,
        &ToAgent::Hello {
            protocol: PROTOCOL + 1,
            version: "new".into(),
        },
    )
    .unwrap();
    let welcome: Option<FromAgent> = read_frame(&mut input).unwrap();
    assert!(matches!(
        welcome,
        Some(FromAgent::Welcome { protocol, .. }) if protocol == PROTOCOL
    ));
    server.join().unwrap().unwrap();
}
