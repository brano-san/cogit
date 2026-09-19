//! Non-blocking logging with a hard 10 MB cap (INV-04).
//!
//! `tracing-appender` and `file-rotate` each ship their own rotation. Wiring both
//! naively gives either double rotation or no size limit at all. The correct shape is
//! `file-rotate` acting as the `io::Write` sink underneath `non_blocking`.

use file_rotate::{ContentLimit, FileRotate, compression::Compression, suffix::AppendCount};
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Hard cap on the live log file.
const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024;
/// How many rotated archives to keep alongside it.
const LOG_ARCHIVE_COUNT: usize = 2;

/// Default filter. Third-party crates are quiet so our own lines stay readable.
const DEFAULT_FILTER: &str = "cogit=debug,cogit_lib=debug,git_engine=debug,diff_engine=debug,\
graph_engine=debug,fs_watcher=debug,app_state=debug,gix=warn,notify=warn,tauri=info";

/// Installs the global subscriber.
///
/// The returned [`WorkerGuard`] **must** be kept alive for the lifetime of the process:
/// dropping it flushes the buffer, and losing it silently truncates the tail of the log.
pub fn init(log_dir: &Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;

    let rotating = FileRotate::new(
        log_dir.join("cogit.log"),
        AppendCount::new(LOG_ARCHIVE_COUNT),
        ContentLimit::Bytes(LOG_SIZE_LIMIT),
        Compression::None,
        None,
    );

    let (writer, guard) = tracing_appender::non_blocking(rotating);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok(guard)
}
