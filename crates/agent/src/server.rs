use crate::frame::{FromAgent, PROTOCOL, ToAgent, invalid, read_frame, write_frame};
use git_engine::NetworkStop;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read, Write};

pub fn serve<R, W, H>(mut input: R, output: W, handler: H) -> io::Result<()>
where
    R: Read,
    W: Write + Send,
    H: Fn(Value, &NetworkStop) -> Result<Value, Value> + Sync,
{
    let Some(ToAgent::Hello { protocol, version }) = read_frame(&mut input)? else {
        return Err(invalid("the first frame is not a hello"));
    };
    let output = Mutex::new(output);
    send(
        &output,
        &FromAgent::Welcome {
            protocol: PROTOCOL,
            version: crate::VERSION.to_owned(),
        },
    )?;
    if protocol != PROTOCOL {
        tracing::error!(protocol, version, "a client of another protocol");
        return Ok(());
    }
    tracing::info!(version, "client connected");
    let running: Mutex<HashMap<u64, NetworkStop>> = Mutex::default();
    std::thread::scope(|scope| {
        let read = read_calls(
            &mut input,
            |id, request| {
                let stop = NetworkStop::default();
                running.lock().insert(id, stop.clone());
                let (output, running, handler) = (&output, &running, &handler);
                scope.spawn(move || {
                    let outcome = handler(request, &stop);
                    running.lock().remove(&id);
                    if let Err(err) = send(output, &FromAgent::Reply { id, outcome }) {
                        tracing::error!(error = ?err, id, context = "agent reply");
                    }
                });
            },
            |id| {
                if let Some(stop) = running.lock().get(&id) {
                    stop.stop();
                }
            },
        );
        for stop in running.lock().values() {
            stop.stop();
        }
        read
    })
}

fn read_calls<R: Read>(
    input: &mut R,
    mut call: impl FnMut(u64, Value),
    cancel: impl Fn(u64),
) -> io::Result<()> {
    while let Some(frame) = read_frame(input)? {
        match frame {
            ToAgent::Call { id, request } => call(id, request),
            ToAgent::Cancel { id } => cancel(id),
            ToAgent::Hello { .. } => return Err(invalid("a second hello")),
        }
    }
    Ok(())
}

fn send<W: Write>(output: &Mutex<W>, frame: &FromAgent) -> io::Result<()> {
    write_frame(&mut *output.lock(), frame)
}
