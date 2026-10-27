#![allow(clippy::unwrap_used)]

use agent::{
    AgentError, Call, Client, FromAgent, PROTOCOL, ToAgent, read_frame, serve, write_frame,
};
use git_engine::NetworkStop;
use serde_json::{Value, json};
use std::io::{self, PipeReader, PipeWriter};
use std::sync::{Arc, Barrier, mpsc};
use std::thread::{self, JoinHandle};

type Handler = Box<dyn Fn(Value, &Call) -> Result<Value, Value> + Send + Sync>;

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
    let server = thread::spawn(move || serve(wires.agent_in, wires.agent_out, |_| {}, handler));
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
    let (client, _server) = pair(Box::new(move |_, call: &Call| {
        started_tx.lock().send(()).unwrap();
        while !call.stop().is_stopped() {
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
            |_| {},
            move |_, call: &Call| {
                started_tx.lock().send(()).unwrap();
                while !call.stop().is_stopped() {
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
    let server =
        thread::spawn(move || serve(wires.agent_in, wires.agent_out, |_| {}, |r, _| Ok(r)));
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

#[test]
fn the_chunks_of_a_call_arrive_in_order_before_its_answer() {
    let (client, _server) = pair(Box::new(|_, call| {
        for n in 0..50 {
            assert!(call.chunk(json!(n)));
        }
        Ok(json!("done"))
    }));
    let mut seen = Vec::new();
    let answer = client
        .call_streaming(json!("walk"), &|| false, &mut |chunk| seen.push(chunk))
        .unwrap();
    assert_eq!(answer, Ok(json!("done")));
    assert_eq!(seen, (0..50).map(|n| json!(n)).collect::<Vec<_>>());
}

#[test]
fn an_event_reaches_the_client_without_a_call() {
    let wires = wires();
    let (heard_tx, heard) = mpsc::channel();
    thread::spawn(move || {
        serve(
            wires.agent_in,
            wires.agent_out,
            |events| {
                assert!(events.send(json!({ "changed": "index" })));
            },
            |r, _| Ok(r),
        )
    });
    let heard_tx = parking_lot::Mutex::new(heard_tx);
    let _client = Client::connect_with_events(wires.client_in, wires.client_out, move |event| {
        heard_tx.lock().send(event).ok();
    })
    .unwrap();
    assert_eq!(
        heard
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap(),
        json!({ "changed": "index" })
    );
}

#[test]
fn a_cancel_gets_through_a_stream_that_never_pauses() {
    let (client, _server) = pair(Box::new(|_, call| {
        while !call.stop().is_stopped() {
            call.chunk(json!("row"));
        }
        Err(json!("canceled"))
    }));
    let seen = std::cell::Cell::new(0);
    let answer = client
        .call_streaming(json!("walk"), &|| seen.get() >= 3, &mut |_| {
            seen.set(seen.get() + 1);
        })
        .unwrap();
    assert_eq!(answer, Err(json!("canceled")));
}
