use serde_json::{Value, json};
use std::io::{self, BufReader, BufWriter};
use std::process::ExitCode;

#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() != Some("serve") {
        return ExitCode::from(2);
    }
    let input = BufReader::new(io::stdin());
    let output = BufWriter::new(io::stdout());
    match agent::serve(input, output, |request: Value, _| {
        Err(json!(format!("unknown request {request}")))
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = ?err, context = "agent serve");
            ExitCode::FAILURE
        }
    }
}
