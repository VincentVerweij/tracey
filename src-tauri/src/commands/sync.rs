//! T070 — sync_configure: validate URI, store in OS keychain, run migrations, enable sync
//! T074 — sync_get_status, sync_trigger: status query and immediate sync trigger

use tauri::State;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use crate::commands::AppState;
use crate::services::sync_service;

// ─────────────────────────────────────────────────────────────
// T070 — sync_configure
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SyncConfigureRequest {
    pub connection_uri: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct SyncConfigureResponse {
    pub connected: bool,
}

#[tauri::command]
pub async fn sync_configure(
    state: State<'_, AppState>,
    request: SyncConfigureRequest,
) -> Result<SyncConfigureResponse, String> {
    if !request.enabled {
        // Disable sync: remove URI from keychain, clear flag in DB
        let _ = clear_keychain_uri(); // best-effort — don't fail if already missing

        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE user_preferences SET external_db_enabled = 0, external_db_uri_stored = 0 WHERE id = 1",
                [],
            )
            .map_err(|e| format!("disable sync failed: {e}"))?;
        }

        {
            let mut ss = state.sync_state.lock().map_err(|e| e.to_string())?;
            ss.connected = false;
            ss.last_error = None;
            ss.cached_uri = None;
        }

        log::info!("[sync_configure] Sync disabled");
        return Ok(SyncConfigureResponse { connected: false });
    }

    // Validate URI format (must start with postgres:// or postgresql://)
    let uri = request.connection_uri.trim().to_string();
    if !uri.starts_with("postgres://") && !uri.starts_with("postgresql://") {
        return Err("invalid_uri: must begin with postgres:// or postgresql://".to_string());
    }

    // Try connecting and running migrations
    let connect_result = sync_service::connect_and_migrate(&uri).await;

    match connect_result {
        Err(e) => {
            {
                let mut ss = state.sync_state.lock().map_err(|err| err.to_string())?;
                ss.connected = false;
                ss.last_error = Some(e.clone());
            }
            log::warn!("[sync_configure] Connection failed: {}", e);
            Err(format!("connection_failed: {e}"))
        }
        Ok(()) => {
            // Store URI securely in OS keychain (best-effort; cache is the live source of truth)
            if let Err(e) = store_keychain_uri(&uri) {
                log::warn!("[sync_configure] Keychain store failed (will use in-memory cache only): {}", e);
            }

            // Update local preferences
            {
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                conn.execute(
                    "UPDATE user_preferences SET external_db_enabled = 1, external_db_uri_stored = 1 WHERE id = 1",
                    [],
                )
                .map_err(|e| format!("enable sync in prefs failed: {e}"))?;
            }

            {
                let mut ss = state.sync_state.lock().map_err(|e| e.to_string())?;
                ss.connected = true;
                ss.last_error = None;
                // Cache URI in memory for the sync loop and sync_trigger
                ss.cached_uri = Some(uri.clone());
                // Reset cursor so the first cycle syncs ALL historical records
                ss.last_sync_cursor = None;
            }

            // Wake sync loop for an immediate initial sync
            state.sync_notify.notify_one();

            log::info!("[sync_configure] Sync configured and enabled");
            Ok(SyncConfigureResponse { connected: true })
        }
    }
}

// ─────────────────────────────────────────────────────────────
// T074 — sync_get_status
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SyncStatusResponse {
    pub enabled: bool,
    pub connected: bool,
    pub pending_queue_size: i64,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

#[tauri::command]
pub fn sync_get_status(state: State<'_, AppState>) -> Result<SyncStatusResponse, String> {
    let enabled = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let enabled: bool = conn.query_row(
            "SELECT external_db_enabled FROM user_preferences LIMIT 1",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false);
        enabled
    };

    let pending_queue_size: i64 = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT COUNT(*) FROM sync_queue", [], |row| row.get(0))
            .unwrap_or(0)
    };

    let (connected, last_sync_at, last_error) = {
        let ss = state.sync_state.lock().map_err(|e| e.to_string())?;
        (ss.connected, ss.last_sync_at.clone(), ss.last_error.clone())
    };

    Ok(SyncStatusResponse {
        enabled,
        connected,
        pending_queue_size,
        last_sync_at,
        last_error,
    })
}

// ─────────────────────────────────────────────────────────────
// T074 — sync_trigger
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SyncTriggerResponse {
    pub synced_records: i64,
    pub errors: i64,
}

#[tauri::command]
pub async fn sync_trigger(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<SyncTriggerResponse, String> {
    // Read URI from in-memory cache first; fall back to keychain for app-restart persistence
    let uri = {
        let ss = state.sync_state.lock().map_err(|e| e.to_string())?;
        ss.cached_uri.clone()
    };
    let uri = match uri {
        Some(u) => u,
        None => read_keychain_uri().map_err(|e| format!("sync not configured: {e}"))?,
    };

    // Always run a full scan (cursor = None = from epoch). The background loop handles
    // incremental cursor-based sync; a manual "Sync now" should push everything.
    let result = sync_service::run_sync_cycle_inline(&app, &state.db, &uri, None).await;

    match result {
        Ok((synced, errors, new_cursor)) => {
            let now = Utc::now().to_rfc3339();
            {
                let mut ss = state.sync_state.lock().map_err(|e| e.to_string())?;
                ss.connected = true;
                ss.last_sync_at = Some(now);
                ss.last_sync_cursor = Some(new_cursor);
                ss.last_error = None;
            }
            Ok(SyncTriggerResponse {
                synced_records: synced,
                errors,
            })
        }
        Err(e) => {
            {
                let mut ss = state.sync_state.lock().map_err(|e2| e2.to_string())?;
                ss.connected = false;
                ss.last_error = Some(e.clone());
            }
            Err(e)
        }
    }
}

// ─────────────────────────────────────────────────────────────
// OS keychain helpers (keyring crate v3)
// ─────────────────────────────────────────────────────────────

const KEYRING_SERVICE: &str = "tracey";
const KEYRING_USER: &str = "external_db_uri";

pub(crate) fn store_keychain_uri(uri: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("keychain entry create failed: {e}"))?;
    entry
        .set_password(uri)
        .map_err(|e| format!("keychain store failed: {e}"))
}

pub(crate) fn read_keychain_uri() -> Result<String, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("keychain entry create failed: {e}"))?;
    entry
        .get_password()
        .map_err(|e| format!("keychain read failed: {e}"))
}

fn clear_keychain_uri() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("keychain entry create failed: {e}"))?;
    entry
        .delete_credential()
        .map_err(|e| format!("keychain delete failed: {e}"))
}
