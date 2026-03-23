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
    pub resumed_entry_id: Option<String>,
    pub resumed_started_at: Option<String>,
}

/// Info captured from the running timer before it is stopped — used to auto-resume after idle.
struct RunningTimerInfo {
    description: String,
    project_id: Option<String>,
    task_id: Option<String>,
    tag_ids: Vec<String>,
}

/// Per-insert transactional context — device ID and timestamp shared across all
/// inserts within a single request handler.
struct WriteCtx {
    device_id: String,
    now: String,
}

/// Data for a new completed time entry (ended_at known).
struct NewEntry<'a> {
    id: &'a str,
    description: &'a str,
    project_id: Option<&'a str>,
    task_id: Option<&'a str>,
    started_at: &'a str,
    ended_at: &'a str,
    is_break: bool,
}

#[tauri::command]
pub fn idle_resolve(
    state: State<'_, AppState>,
    request: IdleResolveRequest,
) -> Result<IdleResolveResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ctx = WriteCtx {
        device_id: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string()),
        now: Utc::now().to_rfc3339(),
    };

    match request.resolution.as_str() {
        "keep" => {
            // No-op: timer keeps running, no new entries
            Ok(IdleResolveResponse { created_entry_id: None, resumed_entry_id: None, resumed_started_at: None })
        }

        "break" => {
            let info = stop_running_timer_at(&conn, &request.idle_started_at, ctx.now.as_str())?;

            // Insert a break entry for the idle period
            let break_id = Ulid::new().to_string();
            insert_entry(&conn, &NewEntry {
                id: &break_id,
                description: "Break",
                project_id: None,
                task_id: None,
                started_at: &request.idle_started_at,
                ended_at: &request.idle_ended_at,
                is_break: true,
            }, &ctx)?;

            let (resumed_entry_id, resumed_started_at) = if let Some(i) = info {
                let rid = insert_running_entry(&conn, &i, &request.idle_ended_at, &ctx)?;
                (Some(rid), Some(request.idle_ended_at.clone()))
            } else { (None, None) };

            Ok(IdleResolveResponse { created_entry_id: Some(break_id), resumed_entry_id, resumed_started_at })
        }

        "meeting" => {
            let info = stop_running_timer_at(&conn, &request.idle_started_at, ctx.now.as_str())?;

            let meeting_id = Ulid::new().to_string();
            insert_entry(&conn, &NewEntry {
                id: &meeting_id,
                description: "Meeting",
                project_id: None,
                task_id: None,
                started_at: &request.idle_started_at,
                ended_at: &request.idle_ended_at,
                is_break: false,
            }, &ctx)?;

            let (resumed_entry_id, resumed_started_at) = if let Some(i) = info {
                let rid = insert_running_entry(&conn, &i, &request.idle_ended_at, &ctx)?;
                (Some(rid), Some(request.idle_ended_at.clone()))
            } else { (None, None) };

            Ok(IdleResolveResponse { created_entry_id: Some(meeting_id), resumed_entry_id, resumed_started_at })
        }

        "specify" => {
            let details = request.entry_details
                .ok_or_else(|| "specify requires entry_details".to_string())?;

            let info = stop_running_timer_at(&conn, &request.idle_started_at, ctx.now.as_str())?;

            let entry_id = Ulid::new().to_string();
            insert_entry(&conn, &NewEntry {
                id: &entry_id,
                description: &details.description,
                project_id: details.project_id.as_deref(),
                task_id: details.task_id.as_deref(),
                started_at: &request.idle_started_at,
                ended_at: &request.idle_ended_at,
                is_break: false,
            }, &ctx)?;

            // Insert tag associations for the idle (specify) entry
            for tag_id in &details.tag_ids {
                conn.execute(
                    "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
                    params![entry_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }

            // Resume the original pre-idle activity
            let (resumed_entry_id, resumed_started_at) = if let Some(i) = info {
                let rid = insert_running_entry(&conn, &i, &request.idle_ended_at, &ctx)?;
                (Some(rid), Some(request.idle_ended_at.clone()))
            } else { (None, None) };

            Ok(IdleResolveResponse { created_entry_id: Some(entry_id), resumed_entry_id, resumed_started_at })
        }

        other => Err(format!("unknown resolution: {}", other)),
    }
}

/// Stop the running timer at `ended_at`. Returns info about the stopped entry so it can be
/// resumed after the idle period. Returns `None` if no timer was running.
fn stop_running_timer_at(
    conn: &rusqlite::Connection,
    ended_at: &str,
    modified_at: &str,
) -> Result<Option<RunningTimerInfo>, String> {
    let running = conn.query_row(
        "SELECT id, description, project_id, task_id \
         FROM time_entries WHERE ended_at IS NULL LIMIT 1",
        [],
        |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        )),
    ).ok();

    let Some((id, description, project_id, task_id)) = running else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT tag_id FROM time_entry_tags WHERE time_entry_id = ?1"
    ).map_err(|e| e.to_string())?;
    let tag_ids: Vec<String> = stmt
        .query_map(params![&id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    conn.execute(
        "UPDATE time_entries SET ended_at = ?1, modified_at = ?2 WHERE id = ?3",
        params![ended_at, modified_at, &id],
    ).map_err(|e| e.to_string())?;

    Ok(Some(RunningTimerInfo { description, project_id, task_id, tag_ids }))
}

/// Insert a new running entry (ended_at = NULL). Used to auto-resume the pre-idle activity.
/// Returns the new entry id.
fn insert_running_entry(
    conn: &rusqlite::Connection,
    info: &RunningTimerInfo,
    started_at: &str,
    ctx: &WriteCtx,
) -> Result<String, String> {
    let id = Ulid::new().to_string();
    conn.execute(
        "INSERT INTO time_entries \
            (id, description, project_id, task_id, started_at, ended_at, is_break, device_id, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, ?6, ?7, ?8)",
        params![id, info.description, info.project_id.as_deref(), info.task_id.as_deref(), started_at, ctx.device_id, ctx.now, ctx.now],
    ).map_err(|e| format!("insert_running_entry failed: {}", e))?;

    for tag_id in &info.tag_ids {
        conn.execute(
            "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
            params![id, tag_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(id)
}

/// Insert a completed time entry (started_at and ended_at both known).
/// Matches the column list in 001_initial_schema.sql exactly, including device_id.
fn insert_entry(
    conn: &rusqlite::Connection,
    entry: &NewEntry<'_>,
    ctx: &WriteCtx,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO time_entries \
            (id, description, project_id, task_id, started_at, ended_at, is_break, device_id, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![entry.id, entry.description, entry.project_id, entry.task_id, entry.started_at, entry.ended_at, entry.is_break, ctx.device_id, ctx.now, ctx.now],
    ).map_err(|e| format!("insert failed: {}", e))?;
    Ok(())
}
