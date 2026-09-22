use app_state::logging::{SessionLog, log_filter, read_log_level, start_label};
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// A panic in a windowed process has nowhere to go: there is no console for stderr, and
/// the non-blocking writer loses whatever it still holds when the process dies. Its own
/// file, written and flushed inline, survives an abort.
pub fn install_panic_hook(log_dir: &Path) {
    let path: PathBuf = log_dir.join("panic.log");
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        write_panic(&path, &format!("{info}\n{backtrace}"));
        tracing::error!(panic = %info, "panic");
        previous(info);
    }));
}

fn write_panic(path: &Path, report: &str) {
    use std::io::Write as _;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "--- {report}");
        let _ = file.flush();
    }
}

/// The returned guard must outlive the process or the tail of the log is lost. The path
/// is this run's first file, which the About window names and Open log reveals.
pub fn init(log_dir: &Path, config_dir: &Path) -> anyhow::Result<(WorkerGuard, PathBuf)> {
    let log = SessionLog::start(log_dir, &start_label())?;
    let path = log.path();
    let (writer, guard) = tracing_appender::non_blocking(log);

    // `RUST_LOG` wins: a developer overriding the level should not have to open the app.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_filter(read_log_level(config_dir).as_deref())));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok((guard, path))
}
