// logger.rs – composite logger: env_logger (stderr) + optional log file.

use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

// ── File-logger globals ───────────────────────────────────────────────────────

/// Whether writing to the log file is currently enabled.
static LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Numeric severity threshold for the log file.
/// 1 = error, 2 = warning, 3 = info, 4 = trace
static LOG_LEVEL: AtomicU8 = AtomicU8::new(3);

/// The open log-file handle (created once at startup, always next to the exe).
static LOG_FILE: OnceLock<Mutex<BufWriter<std::fs::File>>> = OnceLock::new();

// ── MultiLogger ───────────────────────────────────────────────────────────────

/// Composite logger: forwards every `log::*!()` call to both
/// `env_logger` (stderr, respects `RUST_LOG`) and our log file (when enabled).
struct MultiLogger {
    inner: env_logger::Logger,
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // Always return true so the framework sends every record here;
        // we filter for stderr inside inner.log() and for the file via LOG_LEVEL.
        let _ = metadata;
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        // 1. Stderr via env_logger (filtered by RUST_LOG env var).
        if self.inner.enabled(record.metadata()) {
            self.inner.log(record);
        }

        // 2. Log file – only when enabled and the record's level is within the
        //    user-configured threshold.
        if LOG_ENABLED.load(Ordering::Relaxed) {
            let record_u8 = log_level_to_u8(record.level());
            if record_u8 <= LOG_LEVEL.load(Ordering::Relaxed) {
                if let Some(file_mutex) = LOG_FILE.get() {
                    if let Ok(mut writer) = file_mutex.lock() {
                        let ts = chrono::Utc::now().to_rfc3339();
                        let level = record.level().as_str().to_uppercase();
                        let msg = serde_json::to_string(&record.args().to_string())
                            .unwrap_or_else(|_| "\"\"".to_string());
                        let component = record.target();
                        let line = format!(
                            r#"{{"ts":"{ts}","level":"{level}","component":"{component}","message":{msg}}}"#
                        );
                        let _ = writeln!(writer, "{}", line);
                        let _ = writer.flush();
                    }
                }
            }
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

fn log_level_to_u8(level: log::Level) -> u8 {
    match level {
        log::Level::Error => 1,
        log::Level::Warn  => 2,
        log::Level::Info  => 3,
        log::Level::Debug => 4,
        log::Level::Trace => 4,
    }
}

// ── Public initialisation API ─────────────────────────────────────────────────

/// Register the composite logger as the global `log` logger.
/// **Must be called before any `log::*!()` macro use, and before `db::open()`.**
/// Do NOT call `env_logger::init()` anywhere — this function replaces it.
pub fn init_early_logger() {
    let inner = env_logger::Builder::from_default_env().build();
    // Use LevelFilter::Trace so the framework dispatches every record here;
    // env_logger and the file writer each apply their own level filters.
    let logger = Box::new(MultiLogger { inner });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
}

/// Open (or create) the log file and apply the persisted enabled/level settings.
/// Called once after the DB is open and preferences have been read.
/// The file stays open for the lifetime of the process; `update_logging_settings`
/// can toggle writing and change the level without reopening it.
pub fn init_file_logger(enabled: bool, level: &str, log_path: &std::path::Path) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
    LOG_LEVEL.store(level_str_to_u8(level), Ordering::Relaxed);

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        Ok(file) => {
            // OnceLock – ignore the error if already initialised (not expected in practice).
            let _ = LOG_FILE.set(Mutex::new(BufWriter::new(file)));
        }
        Err(e) => {
            log::error!(
                "[logger] Could not open log file {:?}: {}",
                log_path, e
            );
        }
    }
}

/// Update the in-memory logging settings without re-opening the file.
/// Called by `preferences_update` so changes take effect immediately.
pub fn update_logging_settings(enabled: bool, level: &str) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
    LOG_LEVEL.store(level_str_to_u8(level), Ordering::Relaxed);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn level_str_to_u8(level: &str) -> u8 {
    // Accepts the user-facing level names stored in user_preferences.log_level
    match level.to_lowercase().as_str() {
        "error"            => 1,
        "warning" | "warn" => 2,
        "info"             => 3,
        "trace"            => 4,
        _                  => 3,
    }
}
