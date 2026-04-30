#![allow(dead_code)] // Structured logger scaffolded for T082 deny-list redaction — not yet wired.

use serde::Serialize;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

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

// ── File-logger globals ───────────────────────────────────────────────────────

/// Whether writing to the log file is currently enabled.
static LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Numeric severity threshold for the log file.
/// 1 = error, 2 = warning, 3 = info, 4 = trace
static LOG_LEVEL: AtomicU8 = AtomicU8::new(3);

/// The open log-file handle (created once at startup, always next to the exe).
static LOG_FILE: OnceLock<Mutex<BufWriter<std::fs::File>>> = OnceLock::new();

fn level_str_to_u8(level: &str) -> u8 {
    // Accepts the user-facing level names stored in user_preferences.log_level
    match level.to_lowercase().as_str() {
        "error"           => 1,
        "warning" | "warn" => 2,
        "info"            => 3,
        "trace"           => 4,
        _                 => 3,
    }
}

fn event_level_to_u8(level: &str) -> u8 {
    // Accepts the uppercase short-form levels used by log_event()
    match level {
        "ERROR"            => 1,
        "WARN" | "WARNING" => 2,
        "INFO"             => 3,
        "TRACE"            => 4,
        _                  => 3,
    }
}

/// Open (or create) the log file and store the initial enabled/level settings.
/// Should be called once at startup. The file is kept open so that later calls
/// to `update_logging_settings` can toggle writing without re-opening the file.
pub fn init_file_logger(enabled: bool, level: &str, log_path: &std::path::Path) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
    LOG_LEVEL.store(level_str_to_u8(level), Ordering::Relaxed);

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        Ok(file) => {
            // Ignore the error if already initialised (should not happen in practice).
            let _ = LOG_FILE.set(Mutex::new(BufWriter::new(file)));
        }
        Err(e) => {
            eprintln!(
                r#"{{"ts":"{}","level":"ERROR","component":"logger","event":"log_file_open_failed","detail":{{"path":{},"error":{}}}}}"#,
                chrono::Utc::now().to_rfc3339(),
                serde_json::to_string(&log_path.to_string_lossy().as_ref()).unwrap_or_default(),
                serde_json::to_string(&e.to_string()).unwrap_or_default(),
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

// ── Core log function ─────────────────────────────────────────────────────────

/// Log a structured event. `component` = Rust module (e.g. "platform", "db", "commands").
/// `event` = what happened. `detail` = optional extra fields.
///
/// ⚠️ SECURITY: Any string values in `detail` are run through redact() before serialization.
/// The deny-list is loaded from user_preferences at startup and stored in DENY_LIST.
pub fn log_event(level: &str, component: &str, event: &str, detail: Option<serde_json::Value>) {
    let ts = chrono::Utc::now().to_rfc3339();
    // Redact sensitive values from detail before logging
    let safe_detail = detail.map(redact_value);
    let entry = LogEntry {
        ts,
        level,
        component,
        event,
        trace_id: None,
        detail: safe_detail,
    };

    if let Ok(json) = serde_json::to_string(&entry) {
        // Always echo to stderr — Tauri captures stdout; structured logs go to stderr
        eprintln!("{}", json);

        // Write to the log file when enabled and severity is sufficient
        if LOG_ENABLED.load(Ordering::Relaxed) {
            let msg_level       = event_level_to_u8(level);
            let configured_level = LOG_LEVEL.load(Ordering::Relaxed);
            if msg_level <= configured_level {
                if let Some(file_mutex) = LOG_FILE.get() {
                    if let Ok(mut writer) = file_mutex.lock() {
                        let _ = writeln!(writer, "{}", json);
                        let _ = writer.flush();
                    }
                }
            }
        }
    }
}

// ── Deny-list ────────────────────────────────────────────────────────────────

/// Deny-list of process names / window title fragments that must never appear in logs.
/// Populated at startup from user_preferences.process_deny_list_json.
static DENY_LIST: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

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
