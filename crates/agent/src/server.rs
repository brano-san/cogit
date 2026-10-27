use crate::frame::{FromAgent, PROTOCOL, ToAgent, invalid, read_frame, write_frame};
use git_engine::NetworkStop;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::Arc;

type Outbox = Arc<Mutex<dyn Write + Send>>;

#[derive(Clone)]
pub struct Events(Outbox);

impl std::fmt::Debug for Events {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Events")
    }
}

impl Events {
    pub fn send(&self, event: Value) -> bool {
        send(&self.0, &FromAgent::Event { event })
            .inspect_err(|err| tracing::error!(error = ?err, context = "agent event"))
            .is_ok()
    }
}

#[derive(Debug)]
pub struct Call {
    id: u64,
    stop: NetworkStop,
    outbox: Events,
}

impl Call {
    #[must_use]
    pub fn stop(&self) -> &NetworkStop {
        &self.stop
    }

    pub fn chunk(&self, data: Value) -> bool {
        send(&self.outbox.0, &FromAgent::Chunk { id: self.id, data })
            .inspect_err(|err| tracing::error!(error = ?err, id = self.id, context = "agent chunk"))
            .is_ok()
    }
}

pub fn serve<R, W, S, H>(mut input: R, output: W, on_start: S, handler: H) -> io::Result<()>
where
    R: Read,
    W: Write + Send + 'static,
    S: FnOnce(Events),
    H: Fn(Value, &Call) -> Result<Value, Value> + Sync,
{
    let Some(ToAgent::Hello { protocol, version }) = read_frame(&mut input)? else {
        return Err(invalid("the first frame is not a hello"));
    };
    let outbox: Outbox = Arc::new(Mutex::new(output));
    send(
        &outbox,
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
    on_start(Events(outbox.clone()));
    let running: Mutex<HashMap<u64, NetworkStop>> = Mutex::default();
    std::thread::scope(|scope| {
        let read = read_calls(
            &mut input,
            |id, request| {
                let call = Call {
                    id,
                    stop: NetworkStop::default(),
                    outbox: Events(outbox.clone()),
                };
                running.lock().insert(id, call.stop.clone());
                let (running, handler) = (&running, &handler);
                scope.spawn(move || {
                    let outcome = handler(request, &call);
                    running.lock().remove(&id);
                    if let Err(err) = send(&call.outbox.0, &FromAgent::Reply { id, outcome }) {
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

fn send(output: &Mutex<dyn Write + Send>, frame: &FromAgent) -> io::Result<()> {
    write_frame(&mut *output.lock(), frame)
}
