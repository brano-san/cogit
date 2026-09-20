use app_state::logging::{log_filter, read_log_level, rotating_writer};
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// The returned guard must outlive the process or the tail of the log is lost.
pub fn init(log_dir: &Path, config_dir: &Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;

    let (writer, guard) = tracing_appender::non_blocking(rotating_writer(log_dir));

    // `RUST_LOG` wins: a developer overriding the level should not have to open the app.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_filter(read_log_level(config_dir).as_deref())));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok(guard)
}
