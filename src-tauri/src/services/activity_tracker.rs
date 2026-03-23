//! T082 — Window Activity Tracker
//! Polls foreground window every 1 second.
//! Applies process deny-list before any storage write.
//! MutexGuard ALWAYS dropped before any .await point (inner block pattern).
//!
//! T083 note: External sync of window_activity_records is already handled by
//! `sync_service.rs` (queries `synced_at IS NULL` rows every 30 seconds).
//! No separate flush loop is needed here.

use tauri::{AppHandle, Manager};
use chrono::Utc;
use ulid::Ulid;

use crate::commands::AppState;

fn new_id() -> String {
    Ulid::new().to_string()
}

/// Start the window activity polling loop.
/// Calls `state.platform.get_foreground_window_info()` every second.
/// On window change: applies process deny-list, writes to `window_activity_records`.
/// The first tick always counts as a window change (initial state is None).
/// Call once from lib.rs `.setup()`.
pub fn start_activity_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Track last seen window as (process_name, title).
        // None = first tick / no window.
        let mut last_window: Option<(String, String)> = None;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // All synchronous work inside this block.
            // Returns Some(new_last_window) when last_window should be updated.
            // Returns None when there is nothing to do this tick (no change or DB error).
            // MutexGuard is dropped at the closing brace of this block, before the next .await.
            let update: Option<Option<(String, String)>> = {
                let state = app.state::<AppState>();

                // 1. Query foreground window from platform hooks (no DB lock needed here)
                let current_window = state.platform.get_foreground_window_info();
                let current_key = current_window
                    .as_ref()
                    .map(|w| (w.process_name.clone(), w.title.clone()));

                // 2. Detect window change.
                //    Initial state (last_window = None, current Some) counts as a change.
                let changed = match (&last_window, &current_key) {
                    (None, Some(_)) => true,
                    (Some(prev), Some(cur)) => prev != cur,
                    (Some(_), None) => true, // window disappeared
                    (None, None) => false,
                };

                if !changed {
                    None // no-op tick; skip all DB work
                } else {
                    match state.db.lock() {
                        Err(_) => None, // DB unavailable; retry next tick; last_window unchanged
                        Ok(conn) => {
                            // 3. Apply process deny-list BEFORE any storage write (decisions.md)
                            let denied = if let Some(ref win) = current_window {
                                let deny_list_json: String = conn
                                    .query_row(
                                        "SELECT process_deny_list_json \
                                         FROM user_preferences LIMIT 1",
                                        [],
                                        |r| r.get(0),
                                    )
                                    .unwrap_or_else(|_| "[]".to_string());

                                let deny_list: Vec<String> =
                                    serde_json::from_str(&deny_list_json).unwrap_or_default();

                                let process_lower = win.process_name.to_lowercase();
                                deny_list
                                    .iter()
                                    .any(|entry| process_lower.contains(&entry.to_lowercase()))
                            } else {
                                false
                            };

                            // 4. Write to window_activity_records when not denied and a window exists
                            if !denied {
                                if let Some(ref win) = current_window {
                                    let id = new_id();
                                    let now = Utc::now().to_rfc3339();
                                    let device_id = std::env::var("COMPUTERNAME")
                                        .unwrap_or_else(|_| "local".to_string());
                                    // window_handle: composite string identifier (no raw HWND)
                                    let window_handle =
                                        format!("{}:{}", win.process_name, win.title);

                                    let _ = conn.execute(
                                        "INSERT INTO window_activity_records \
                                         (id, process_name, window_title, window_handle, \
                                          recorded_at, device_id) \
                                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                        rusqlite::params![
                                            id,
                                            win.process_name,
                                            win.title,
                                            window_handle,
                                            now,
                                            device_id,
                                        ],
                                    );
                                }
                            }

                            // Always advance last_window tracking, even when denied.
                            // (Denied processes still mark a "real" window change in OS terms.)
                            Some(current_key)
                        }
                    }
                }
            }; // MutexGuard dropped here — NEVER held across an .await point

            // Update last_window state outside the inner block
            if let Some(new_last) = update {
                last_window = new_last;
            }
        }
    });
}
