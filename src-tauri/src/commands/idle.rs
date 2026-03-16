use tauri::State;
use serde::{Deserialize, Serialize};
use rusqlite::params;
use chrono::Utc;
use ulid::Ulid;
use crate::commands::AppState;
use crate::services::idle_service;

// ─────────────────────────────────────────────────────────────
// T033: idle_get_status
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct IdleStatusResponse {
    pub is_idle: bool,
    pub idle_seconds: u64,
    pub idle_since: Option<String>,
}

#[tauri::command]
pub fn idle_get_status(state: State<'_, AppState>) -> Result<IdleStatusResponse, String> {
    let threshold = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT inactivity_timeout_seconds FROM user_preferences LIMIT 1",
            [],
            |r| r.get::<_, i64>(0),
        ).unwrap_or(300)
    };

    let (is_idle, idle_seconds, idle_since) = idle_service::get_current_idle_status(
        state.platform.as_ref(),
        threshold,
    );

    Ok(IdleStatusResponse { is_idle, idle_seconds, idle_since })
}

// ─────────────────────────────────────────────────────────────
// T034: idle_resolve
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EntryDetails {
    pub description: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub tag_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct IdleResolveRequest {
    pub resolution: String,       // "break" | "meeting" | "specify" | "keep"
    pub idle_started_at: String,
    pub idle_ended_at: String,
    pub entry_details: Option<EntryDetails>,
}

#[derive(Serialize)]
pub struct IdleResolveResponse {
    pub created_entry_id: Option<String>,
}

#[tauri::command]
pub fn idle_resolve(
    state: State<'_, AppState>,
    request: IdleResolveRequest,
) -> Result<IdleResolveResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    match request.resolution.as_str() {
        "keep" => {
            // No-op: timer keeps running, no new entries
            Ok(IdleResolveResponse { created_entry_id: None })
        }

        "break" => {
            // Stop the running timer at idle_started_at
            stop_running_timer_at(&conn, &request.idle_started_at, &now)?;

            // Insert a break entry for the idle period
            let break_id = Ulid::new().to_string();
            insert_entry(
                &conn,
                &break_id,
                "Break",
                None,
                None,
                &request.idle_started_at,
                &request.idle_ended_at,
                true,
                &device_id,
                &now,
            )?;

            Ok(IdleResolveResponse { created_entry_id: Some(break_id) })
        }

        "meeting" => {
            // Stop running timer at idle_started_at
            stop_running_timer_at(&conn, &request.idle_started_at, &now)?;

            let meeting_id = Ulid::new().to_string();
            insert_entry(
                &conn,
                &meeting_id,
                "Meeting",
                None,
                None,
                &request.idle_started_at,
                &request.idle_ended_at,
                false,
                &device_id,
                &now,
            )?;

            Ok(IdleResolveResponse { created_entry_id: Some(meeting_id) })
        }

        "specify" => {
            let details = request.entry_details
                .ok_or_else(|| "specify requires entry_details".to_string())?;

            // Stop running timer at idle_started_at
            stop_running_timer_at(&conn, &request.idle_started_at, &now)?;

            let entry_id = Ulid::new().to_string();
            insert_entry(
                &conn,
                &entry_id,
                &details.description,
                details.project_id.as_deref(),
                details.task_id.as_deref(),
                &request.idle_started_at,
                &request.idle_ended_at,
                false,
                &device_id,
                &now,
            )?;

            // Insert tag associations
            for tag_id in &details.tag_ids {
                conn.execute(
                    "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
                    params![entry_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }

            Ok(IdleResolveResponse { created_entry_id: Some(entry_id) })
        }

        other => Err(format!("unknown resolution: {}", other)),
    }
}

/// Stop any running timer, setting ended_at to the given timestamp (not now).
/// Timer stopped at idle_started_at — not at resolution time.
fn stop_running_timer_at(
    conn: &rusqlite::Connection,
    ended_at: &str,
    modified_at: &str,
) -> Result<(), String> {
    let running = conn.query_row(
        "SELECT id FROM time_entries WHERE ended_at IS NULL LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    ).ok();

    if let Some(running_id) = running {
        conn.execute(
            "UPDATE time_entries SET ended_at = ?1, modified_at = ?2 WHERE id = ?3",
            params![ended_at, modified_at, running_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Insert a completed time entry (started_at and ended_at both known).
/// Matches the column list in 001_initial_schema.sql exactly, including device_id.
fn insert_entry(
    conn: &rusqlite::Connection,
    id: &str,
    description: &str,
    project_id: Option<&str>,
    task_id: Option<&str>,
    started_at: &str,
    ended_at: &str,
    is_break: bool,
    device_id: &str,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO time_entries \
            (id, description, project_id, task_id, started_at, ended_at, is_break, device_id, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, description, project_id, task_id, started_at, ended_at, is_break, device_id, now, now],
    ).map_err(|e| format!("insert failed: {}", e))?;
    Ok(())
}
