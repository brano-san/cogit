use file_rotate::{ContentLimit, FileRotate, compression::Compression, suffix::AppendCount};
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024;
const LOG_ARCHIVE_COUNT: usize = 2;

const DEFAULT_FILTER: &str = "cogit=debug,cogit_lib=debug,git_engine=debug,diff_engine=debug,\
graph_engine=debug,fs_watcher=debug,app_state=debug,gix=warn,notify=warn,tauri=info";

/// The returned guard must outlive the process or the tail of the log is lost.
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
