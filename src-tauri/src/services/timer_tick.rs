use tauri::{AppHandle, Emitter, Manager};
use chrono::Utc;
use crate::commands::AppState;

/// Start the timer-tick background task.
/// Call once from lib.rs `.setup()`. Emits `tracey://timer-tick` every second
/// while a timer is running, with `{ elapsed_seconds, entry_id }` payload.
pub fn start_tick_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Query running timer without holding the lock across the await point.
            let tick_payload = {
                let state = app.state::<AppState>();
                let conn = match state.db.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let result = conn.query_row(
                    "SELECT id, started_at FROM time_entries WHERE ended_at IS NULL LIMIT 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                );

                match result {
                    Ok((id, started_at)) => {
                        let elapsed = chrono::DateTime::parse_from_rfc3339(&started_at)
                            .map(|s| (Utc::now() - s.with_timezone(&Utc)).num_seconds().max(0))
                            .unwrap_or(0);
                        Some(serde_json::json!({
                            "elapsed_seconds": elapsed,
                            "entry_id": id
                        }))
                    }
                    Err(_) => None,
                }
            };

            if let Some(payload) = tick_payload {
                let _ = app.emit("tracey://timer-tick", payload);
            }
        }
    });
}
