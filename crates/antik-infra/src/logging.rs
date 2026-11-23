use std::io;
use std::path::Path;
use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Holds the non-blocking writer for the process lifetime. Dropping it would
/// stop log flushing, so it is stored rather than returned.
static GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

pub fn init(log_dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(log_dir)?;
    let appender = tracing_appender::rolling::daily(log_dir, "antik.jsonl");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,antik=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_ansi(false).with_writer(writer))
        .try_init()
        .map_err(|error| io::Error::other(error.to_string()))?;

    if let Ok(mut slot) = GUARD.lock() {
        *slot = Some(guard);
    }
    Ok(())
}
