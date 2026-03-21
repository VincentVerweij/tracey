#![allow(dead_code)] // Structured logger scaffolded for T082 deny-list redaction — not yet wired.

use serde::Serialize;

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
    // Use eprintln for structured logs — Tauri captures stdout; structured logs go to stderr
    if let Ok(json) = serde_json::to_string(&entry) {
        eprintln!("{}", json);
    }
}

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

/// Convenience macros
#[macro_export]
macro_rules! log_info {
    ($component:expr, $event:expr) => {
        crate::services::logger::log_event("INFO", $component, $event, None)
    };
    ($component:expr, $event:expr, $detail:expr) => {
        crate::services::logger::log_event("INFO", $component, $event, Some($detail))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($component:expr, $event:expr) => {
        crate::services::logger::log_event("WARN", $component, $event, None)
    };
}

#[macro_export]
macro_rules! log_err {
    ($component:expr, $event:expr, $detail:expr) => {
        crate::services::logger::log_event("ERROR", $component, $event, Some($detail))
    };
}
