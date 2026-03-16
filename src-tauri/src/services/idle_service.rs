use tauri::{AppHandle, Emitter, Manager};
use chrono::Utc;
use crate::platform::PlatformHooks;
use crate::commands::AppState;

/// Tracks idle state across loop iterations.
struct IdleState {
    is_idle: bool,
    idle_started_at: Option<chrono::DateTime<chrono::Utc>>,
    had_active_timer: bool,
}

/// Start the idle detection background loop.
/// Uses PlatformHooks.get_idle_seconds() — NOT tauri-plugin-system-idle (that plugin doesn't exist).
pub fn start_idle_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut state = IdleState {
            is_idle: false,
            idle_started_at: None,
            had_active_timer: false,
        };

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Read threshold + platform data synchronously (never hold mutex across await)
            let (threshold_secs, idle_secs, has_running_timer) = {
                let app_state = app.state::<AppState>();

                let idle_secs = app_state.platform.get_idle_seconds();

                let conn = match app_state.db.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let threshold: i64 = conn.query_row(
                    "SELECT inactivity_timeout_seconds FROM user_preferences LIMIT 1",
                    [],
                    |r| r.get(0),
                ).unwrap_or(300);

                let running: bool = conn.query_row(
                    "SELECT COUNT(*) > 0 FROM time_entries WHERE ended_at IS NULL",
                    [],
                    |r| r.get(0),
                ).unwrap_or(false);

                (threshold, idle_secs, running)
            };

            let is_now_idle = idle_secs >= threshold_secs as u64;

            // Transition: not idle → idle
            if is_now_idle && !state.is_idle {
                state.is_idle = true;
                state.idle_started_at = Some(Utc::now() - chrono::Duration::seconds(idle_secs as i64));
                state.had_active_timer = has_running_timer;

                // Only emit if a timer was running
                if state.had_active_timer {
                    let idle_since = state.idle_started_at
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_else(|| Utc::now().to_rfc3339());

                    let _ = app.emit("tracey://idle-detected", serde_json::json!({
                        "idle_since": idle_since,
                        "had_active_timer": true
                    }));

                    log::info!("Idle detected at {} after {}s", idle_since, idle_secs);
                }
            }

            // Transition: idle → active
            if !is_now_idle && state.is_idle {
                state.is_idle = false;
                state.idle_started_at = None;
                state.had_active_timer = false;
            }
        }
    });
}

/// Query idle status directly from platform hooks.
/// Used by the idle_get_status command — the loop's internal state is not shared.
pub fn get_current_idle_status(platform: &dyn PlatformHooks, threshold_secs: i64) -> (bool, u64, Option<String>) {
    let idle_secs = platform.get_idle_seconds();
    let is_idle = idle_secs >= threshold_secs as u64;
    let idle_since = if is_idle {
        let since = Utc::now() - chrono::Duration::seconds(idle_secs as i64);
        Some(since.to_rfc3339())
    } else {
        None
    };
    (is_idle, idle_secs, idle_since)
}
