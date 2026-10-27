use app_state::AppState;
use std::io::{self, BufReader, BufWriter};
use std::process::ExitCode;

#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() != Some("serve") {
        return ExitCode::from(2);
    }
    let state = AppState::new();
    let input = BufReader::new(io::stdin());
    let output = BufWriter::new(io::stdout());
    match agent::serve(input, output, |request, stop| {
        app_state::serve_agent(&state, request, stop)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = ?err, context = "agent serve");
            ExitCode::FAILURE
        }
    }
}
