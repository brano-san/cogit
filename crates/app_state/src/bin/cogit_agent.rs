use app_state::AppState;
use std::io::{self, BufReader, BufWriter};
use std::process::ExitCode;
use std::sync::Arc;

#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    if let Some(code) = git_engine::run_as_helper() {
        return ExitCode::from(u8::try_from(code).unwrap_or(1));
    }
    if std::env::args().nth(1).as_deref() != Some("serve") {
        return ExitCode::from(2);
    }
    let state = Arc::new(AppState::new());
    let input = BufReader::new(io::stdin());
    let output = BufWriter::new(io::stdout());
    let forwarded = Arc::clone(&state);
    match agent::serve(
        input,
        output,
        move |events| app_state::forward_agent_events(forwarded, events),
        |request, call| app_state::serve_agent(&state, request, call),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = ?err, context = "agent serve");
            ExitCode::FAILURE
        }
    }
}
