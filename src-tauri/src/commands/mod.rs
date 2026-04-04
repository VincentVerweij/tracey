pub mod activity;
pub mod classification;
pub mod data;
pub mod hierarchy;
pub mod idle;
pub mod screenshot;
pub mod sync;
pub mod timer;

use tauri::State;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::models::UserPreferences;
use crate::platform::PlatformHooks;
use crate::services::classification::tfidf::TfIdfModel;
use crate::services::classification::heuristic::HeuristicRule;

/// Shared classification state: loaded model + rule cache + sample counter.
#[derive(Default)]
pub struct ClassificationState {
    pub model: Option<TfIdfModel>,
    pub rules: Vec<HeuristicRule>,
    pub sample_count_at_last_train: i64,
}

/// Shared sync state updated by the SyncService background loop.
#[derive(Default)]
pub struct SyncState {
    pub connected: bool,
    pub last_sync_at: Option<String>,
    pub last_sync_cursor: Option<String>, // ISO 8601 — scan modified_at >= this
    pub last_error: Option<String>,
    /// URI cached in memory after a successful sync_configure (or restored from keychain at
    /// startup). Avoids a round-trip to the OS credential store on every sync tick.
    pub cached_uri: Option<String>,
}

/// Application-wide shared state holding the DB connection and platform hooks.
/// Wrapped in Mutex so Tauri commands (which run concurrently) get exclusive access.
pub struct AppState {
    pub db: std::sync::Mutex<rusqlite::Connection>,
    pub platform: Arc<dyn PlatformHooks + Send + Sync>,
    /// Shared sync state; updated by the SyncService background loop.
    pub sync_state: Arc<std::sync::Mutex<SyncState>>,
    /// Notify fired to wake the sync background loop for an immediate sync cycle.
    pub sync_notify: Arc<tokio::sync::Notify>,
    pub classification_state: Arc<std::sync::Mutex<ClassificationState>>,
}

// ─────────────────────────────────────────────────────────────
// Sync queue helpers (shared across commands)
// ─────────────────────────────────────────────────────────────

/// Enqueue a delete operation in sync_queue so the SyncService can propagate it
/// to the external Postgres DB. Wired in by T083 (window activity sync).
#[allow(dead_code)]
pub fn enqueue_delete(
    conn: &rusqlite::Connection,
    table_name: &str,
    record_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sync_queue (table_name, record_id, operation, queued_at, attempts)
         VALUES (?1, ?2, 'delete', ?3, 0)",
        rusqlite::params![table_name, record_id, now],
    )
    .map_err(|e| format!("enqueue_delete failed: {e}"))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// Preferences
// ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn preferences_get(state: State<'_, AppState>) -> Result<UserPreferences, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let prefs = conn.query_row(
        "SELECT id, local_timezone, inactivity_timeout_seconds,
                screenshot_interval_seconds, screenshot_retention_days,
                screenshot_storage_path, timer_notification_threshold_hours,
                page_size, external_db_uri_stored, external_db_enabled,
                notification_channels_json, process_deny_list_json
         FROM user_preferences LIMIT 1",
        [],
        |row| {
            Ok(UserPreferences {
                id: row.get(0)?,
                local_timezone: row.get(1)?,
                inactivity_timeout_seconds: row.get(2)?,
                screenshot_interval_seconds: row.get(3)?,
                screenshot_retention_days: row.get(4)?,
                screenshot_storage_path: row.get(5)?,
                timer_notification_threshold_hours: row.get(6)?,
                page_size: row.get(7)?,
                external_db_uri_stored: row.get(8)?,
                external_db_enabled: row.get(9)?,
                notification_channels_json: row.get(10)?,
                process_deny_list_json: row.get(11)?,
            })
        },
    )
    .map_err(|e| format!("preferences_get failed: {}", e))?;

    Ok(prefs)
}

#[derive(Deserialize)]
pub struct PreferencesUpdateRequest {
    pub local_timezone: Option<String>,
    pub inactivity_timeout_seconds: Option<i64>,
    pub screenshot_interval_seconds: Option<i64>,
    pub screenshot_retention_days: Option<i64>,
    pub screenshot_storage_path: Option<String>,
    pub timer_notification_threshold_hours: Option<f64>,
    pub page_size: Option<i64>,
    pub external_db_enabled: Option<bool>,
    pub notification_channels_json: Option<String>,
    pub process_deny_list_json: Option<String>,
}

#[tauri::command]
pub fn preferences_update(
    state: State<'_, AppState>,
    update: PreferencesUpdateRequest,
) -> Result<UserPreferences, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Read current, apply deltas, write all (partial-update via read-modify-write)
    let mut current = conn.query_row(
        "SELECT id, local_timezone, inactivity_timeout_seconds,
                screenshot_interval_seconds, screenshot_retention_days,
                screenshot_storage_path, timer_notification_threshold_hours,
                page_size, external_db_uri_stored, external_db_enabled,
                notification_channels_json, process_deny_list_json
         FROM user_preferences LIMIT 1",
        [],
        |row| {
            Ok(UserPreferences {
                id: row.get(0)?,
                local_timezone: row.get(1)?,
                inactivity_timeout_seconds: row.get(2)?,
                screenshot_interval_seconds: row.get(3)?,
                screenshot_retention_days: row.get(4)?,
                screenshot_storage_path: row.get(5)?,
                timer_notification_threshold_hours: row.get(6)?,
                page_size: row.get(7)?,
                external_db_uri_stored: row.get(8)?,
                external_db_enabled: row.get(9)?,
                notification_channels_json: row.get(10)?,
                process_deny_list_json: row.get(11)?,
            })
        },
    )
    .map_err(|e| format!("preferences_update read failed: {}", e))?;

    // Apply deltas — only fields present in the request are changed
    if let Some(v) = update.local_timezone { current.local_timezone = v; }
    if let Some(v) = update.inactivity_timeout_seconds { current.inactivity_timeout_seconds = v; }
    if let Some(v) = update.screenshot_interval_seconds { current.screenshot_interval_seconds = v; }
    if let Some(v) = update.screenshot_retention_days { current.screenshot_retention_days = v; }
    if update.screenshot_storage_path.is_some() { current.screenshot_storage_path = update.screenshot_storage_path; }
    if let Some(v) = update.timer_notification_threshold_hours { current.timer_notification_threshold_hours = v; }
    if let Some(v) = update.page_size { current.page_size = v; }
    if let Some(v) = update.external_db_enabled { current.external_db_enabled = v; }
    if let Some(v) = update.notification_channels_json { current.notification_channels_json = Some(v); }
    if let Some(v) = update.process_deny_list_json { current.process_deny_list_json = v; }
    // external_db_uri_stored is NOT updated here — managed exclusively by sync_configure command

    conn.execute(
        "UPDATE user_preferences SET
            local_timezone = ?1,
            inactivity_timeout_seconds = ?2,
            screenshot_interval_seconds = ?3,
            screenshot_retention_days = ?4,
            screenshot_storage_path = ?5,
            timer_notification_threshold_hours = ?6,
            page_size = ?7,
            external_db_enabled = ?8,
            notification_channels_json = ?9,
            process_deny_list_json = ?10
         WHERE id = ?11",
        rusqlite::params![
            current.local_timezone,
            current.inactivity_timeout_seconds,
            current.screenshot_interval_seconds,
            current.screenshot_retention_days,
            current.screenshot_storage_path,
            current.timer_notification_threshold_hours,
            current.page_size,
            current.external_db_enabled,
            current.notification_channels_json,
            current.process_deny_list_json,
            current.id,
        ],
    )
    .map_err(|e| format!("preferences_update write failed: {}", e))?;

    Ok(current)
}

// ─────────────────────────────────────────────────────────────
// Health  (shape follows contracts/ipc-commands.md)
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub running: bool,
    pub last_write_at: Option<String>,  // populated by ActivityTracker in later tasks
    pub events_per_sec: f64,            // populated by ActivityTracker in later tasks
    pub memory_mb: f64,                 // populated by metrics service in later tasks
    pub active_errors: Vec<String>,
    pub pending_sync_count: i64,
}

/// Called before tauri::Builder to initialise any health-monitoring state.
/// Currently a placeholder; future tasks will start metric collection here.
pub fn init_health() {}

#[tauri::command]
pub fn health_get(state: State<'_, AppState>) -> HealthResponse {
    let pending_sync_count = match state.db.lock() {
        Ok(conn) => conn
            .query_row("SELECT COUNT(*) FROM sync_queue", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0),
        Err(_) => 0,
    };

    HealthResponse {
        running: true,
        last_write_at: None,
        events_per_sec: 0.0,
        memory_mb: 0.0,
        active_errors: vec![],
        pending_sync_count,
    }
}
