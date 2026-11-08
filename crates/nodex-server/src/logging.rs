//! Logging to the console and to daily files in `<data dir>/logs` (kept for a week), so a
//! problem in the desktop app, which has no console, can still be read afterwards.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Default filter when `RUST_LOG` is not set.
const DEFAULT_FILTER: &str = "nodex=info";

/// Log files kept (one per day).
const KEEP_FILES: usize = 7;

/// Where the log files go.
pub fn logs_dir() -> PathBuf {
    crate::projects::data_dir().join("logs")
}

/// Installs the global subscriber: the console (when `console` is true) and a daily log file.
/// Keep the returned guard alive until exit, or the last lines are lost.
pub fn init(console: bool) -> Option<WorkerGuard> {
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_FILTER.into());
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("nodex")
        .filename_suffix("log")
        .max_log_files(KEEP_FILES)
        .build(logs_dir())
        .ok();
    let (file_layer, guard) = match file {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            (Some(fmt::layer().with_ansi(false).with_writer(writer).with_filter(filter())), Some(guard))
        }
        None => (None, None),
    };
    let console_layer = console.then(|| fmt::layer().with_filter(filter()));
    let _ = tracing_subscriber::registry().with(file_layer).with(console_layer).try_init();
    guard
}
