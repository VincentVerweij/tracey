// logger.rs – composite logger: env_logger (stderr) + optional log file.

use serde::Serialize;
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

// ── Structured log_event (for explicit JSON events with a detail payload) ─────

#[derive(Serialize)]
struct LogEntry<'a> {
    ts: String,
    level: &'a str,
    component: &'a str,
    event: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<serde_json::Value>,
}

/// Log a structured JSON event with an optional `detail` payload.
/// `component` = Rust module.  `event` = what happened.
///
/// ⚠️ SECURITY: String values in `detail` are redacted against the deny-list.
///
/// This goes through the `log` crate (and therefore the MultiLogger) so that
/// both the stderr and the file writers see it.
pub fn log_event(level: &str, component: &str, event: &str, detail: Option<serde_json::Value>) {
    let safe_detail = detail.map(redact_value);
    let entry = LogEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level,
        component,
        event,
        trace_id: None,
        detail: safe_detail,
    };
    if let Ok(json) = serde_json::to_string(&entry) {
        // Route through the standard log crate so MultiLogger handles both
        // stderr and file output uniformly.
        match level {
            "ERROR" => log::error!(target: component, "{}", json),
            "WARN"  => log::warn!(target: component, "{}", json),
            "TRACE" => log::trace!(target: component, "{}", json),
            _       => log::info!(target: component, "{}", json),
        }
    }
}

// ── Deny-list ────────────────────────────────────────────────────────────────

/// Deny-list of process names / window title fragments that must never appear in logs.
/// Populated at startup from user_preferences.process_deny_list_json.
static DENY_LIST: OnceLock<Vec<String>> = OnceLock::new();

pub fn init_deny_list(json_list: &str) {
    let list: Vec<String> = serde_json::from_str(json_list).unwrap_or_default();
    let _ = DENY_LIST.set(list);
}

fn redact_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::String(s) => serde_json::Value::String(redact_str(&s)),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, redact_value(v)))
                .collect(),
        ),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(redact_value).collect())
        }
        other => other,
    }
}

fn redact_str(s: &str) -> String {
    let deny = DENY_LIST.get().map(|v| v.as_slice()).unwrap_or(&[]);
    let lower = s.to_lowercase();
    for pattern in deny {
        if lower.contains(&pattern.to_lowercase()) {
            return "[REDACTED]".to_string();
        }
    }
    s.to_string()
}

// ── Convenience macros ────────────────────────────────────────────────────────

#[macro_export]
macro_rules! log_info {
    ($component:expr, $event:expr) => {
        $crate::services::logger::log_event("INFO", $component, $event, None)
    };
    ($component:expr, $event:expr, $detail:expr) => {
        $crate::services::logger::log_event("INFO", $component, $event, Some($detail))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($component:expr, $event:expr) => {
        $crate::services::logger::log_event("WARN", $component, $event, None)
    };
}

#[macro_export]
macro_rules! log_err {
    ($component:expr, $event:expr, $detail:expr) => {
        $crate::services::logger::log_event("ERROR", $component, $event, Some($detail))
    };
}
