use tauri::State;
use serde::{Deserialize, Serialize};
use rusqlite::params;
use crate::commands::AppState;
use chrono::Utc;
use ulid::Ulid;

fn new_id() -> String {
    Ulid::new().to_string()
}

// ─────────────────────────────────────────────────────────────
// T020: timer_start
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TimerStartRequest {
    pub description: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub tag_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct StoppedEntry {
    pub id: String,
    pub ended_at: String,
}

#[derive(Serialize)]
pub struct TimerStartResponse {
    pub id: String,
    pub started_at: String,
    pub stopped_entry: Option<StoppedEntry>,
}

#[tauri::command]
pub fn timer_start(
    state: State<'_, AppState>,
    request: TimerStartRequest,
) -> Result<TimerStartResponse, String> {
    // Empty description is valid when a project is selected — the project/task context
    // already identifies the work. Description is only mandatory in plain (no-project) mode.
    let empty_without_context =
        request.description.is_empty() && request.project_id.is_none();
    if empty_without_context || request.description.len() > 500 {
        return Err("invalid_description".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    // Stop any currently running timer
    let stopped_entry = stop_running_timer(&conn, &now)?;

    // Validate project/task exist if provided
    if let Some(ref pid) = request.project_id {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                params![pid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("project_not_found".to_string());
        }
    }
    if let Some(ref tid) = request.task_id {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = ?1",
                params![tid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("task_not_found".to_string());
        }
    }
    for tag_id in &request.tag_ids {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tags WHERE id = ?1",
                params![tag_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("tag_not_found".to_string());
        }
    }

    let (id, started_at) = insert_new_timer(
        &conn,
        &request.description,
        request.project_id,
        request.task_id,
        &request.tag_ids,
        &now,
    )?;

    Ok(TimerStartResponse {
        id,
        started_at,
        stopped_entry,
    })
}

/// Stop any running timer (ended_at IS NULL). Returns info about stopped entry if one existed.
fn stop_running_timer(
    conn: &rusqlite::Connection,
    ended_at: &str,
) -> Result<Option<StoppedEntry>, String> {
    let result = conn.query_row(
        "SELECT id FROM time_entries WHERE ended_at IS NULL LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    );

    match result {
        Ok(running_id) => {
            conn.execute(
                "UPDATE time_entries SET ended_at = ?1, modified_at = ?2 WHERE id = ?3",
                params![ended_at, ended_at, running_id],
            )
            .map_err(|e| format!("stop failed: {}", e))?;
            Ok(Some(StoppedEntry {
                id: running_id,
                ended_at: ended_at.to_string(),
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("query failed: {}", e)),
    }
}

/// Insert a new running time entry and its tag associations. Returns (id, started_at).
fn insert_new_timer(
    conn: &rusqlite::Connection,
    description: &str,
    project_id: Option<String>,
    task_id: Option<String>,
    tag_ids: &[String],
    now: &str,
) -> Result<(String, String), String> {
    let id = new_id();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    conn.execute(
        "INSERT INTO time_entries \
            (id, description, project_id, task_id, started_at, ended_at, is_break, device_id, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, ?6, ?7, ?8)",
        params![id, description, project_id, task_id, now, device_id, now, now],
    )
    .map_err(|e| format!("insert failed: {}", e))?;

    for tag_id in tag_ids {
        conn.execute(
            "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
            params![id, tag_id],
        )
        .map_err(|e| format!("tag insert failed: {}", e))?;
    }

    log::info!("insert_new_timer: entry {} started at {}", id, now);
    Ok((id, now.to_string()))
}

// ─────────────────────────────────────────────────────────────
// T021: timer_stop, timer_get_active
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TimerStopResponse {
    pub id: String,
    pub ended_at: String,
}

#[tauri::command]
pub fn timer_stop(state: State<'_, AppState>) -> Result<TimerStopResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    match stop_running_timer(&conn, &now)? {
        Some(stopped) => Ok(TimerStopResponse {
            id: stopped.id,
            ended_at: stopped.ended_at,
        }),
        None => Err("no_active_timer".to_string()),
    }
}

#[derive(Serialize)]
pub struct ActiveTimerResponse {
    pub id: Option<String>,
    pub description: String,
    pub started_at: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub tag_ids: Vec<String>,
}

#[tauri::command]
pub fn timer_get_active(state: State<'_, AppState>) -> Result<ActiveTimerResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let result = conn.query_row(
        "SELECT id, description, started_at, project_id, task_id
         FROM time_entries WHERE ended_at IS NULL LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        },
    );

    match result {
        Ok((id, description, started_at, project_id, task_id)) => {
            let mut stmt = conn
                .prepare("SELECT tag_id FROM time_entry_tags WHERE time_entry_id = ?1")
                .map_err(|e| e.to_string())?;
            let tag_ids: Vec<String> = stmt
                .query_map(params![id], |r| r.get(0))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            Ok(ActiveTimerResponse {
                id: Some(id),
                description,
                started_at,
                project_id,
                task_id,
                tag_ids,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(ActiveTimerResponse {
            id: None,
            description: String::new(),
            started_at: String::new(),
            project_id: None,
            task_id: None,
            tag_ids: vec![],
        }),
        Err(e) => Err(e.to_string()),
    }
}

// ─────────────────────────────────────────────────────────────
// T022: time_entry_list
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TimeEntryListRequest {
    pub page: i64,
    pub page_size: i64,
}

#[derive(Serialize)]
pub struct TimeEntryListItem {
    pub id: String,
    pub description: String,
    pub started_at: String,
    pub ended_at: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub client_name: Option<String>,
    pub task_id: Option<String>,
    pub task_name: Option<String>,
    pub tag_ids: Vec<String>,
    pub tag_names: Vec<String>,
    pub is_break: bool,
}

#[derive(Serialize)]
pub struct TimeEntryListResponse {
    pub entries: Vec<TimeEntryListItem>,
    pub total_count: i64,
    pub has_more: bool,
}

#[tauri::command]
pub fn time_entry_list(
    state: State<'_, AppState>,
    request: TimeEntryListRequest,
) -> Result<TimeEntryListResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let page_size = request.page_size.clamp(1, 200);
    let offset = (request.page - 1).max(0) * page_size;

    let total_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM time_entries WHERE ended_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Schema confirmed: projects has client_id → clients.id; is_break exists in time_entries.
    let mut stmt = conn
        .prepare(
            "SELECT te.id, te.description, te.started_at, te.ended_at,
                    te.project_id, p.name AS project_name, c.name AS client_name,
                    te.task_id, t.name AS task_name, te.is_break
             FROM time_entries te
             LEFT JOIN projects p ON te.project_id = p.id
             LEFT JOIN clients  c ON p.client_id   = c.id
             LEFT JOIN tasks    t ON te.task_id     = t.id
             WHERE te.ended_at IS NOT NULL
             ORDER BY te.started_at DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let mut entries: Vec<TimeEntryListItem> = stmt
        .query_map(params![page_size, offset], |r| {
            Ok((
                r.get::<_, String>(0)?,         // id
                r.get::<_, String>(1)?,         // description
                r.get::<_, String>(2)?,         // started_at
                r.get::<_, String>(3)?,         // ended_at
                r.get::<_, Option<String>>(4)?, // project_id
                r.get::<_, Option<String>>(5)?, // project_name
                r.get::<_, Option<String>>(6)?, // client_name
                r.get::<_, Option<String>>(7)?, // task_id
                r.get::<_, Option<String>>(8)?, // task_name
                r.get::<_, bool>(9)?,           // is_break (INTEGER 0/1 → bool)
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(
            |(id, description, started_at, ended_at, project_id, project_name,
              client_name, task_id, task_name, is_break)| TimeEntryListItem {
                id,
                description,
                started_at,
                ended_at,
                project_id,
                project_name,
                client_name,
                task_id,
                task_name,
                tag_ids: vec![],
                tag_names: vec![],
                is_break,
            },
        )
        .collect();

    // Fetch tags for each entry
    for entry in &mut entries {
        let mut tag_stmt = conn
            .prepare(
                "SELECT tet.tag_id, tag.name
                 FROM time_entry_tags tet
                 JOIN tags tag ON tet.tag_id = tag.id
                 WHERE tet.time_entry_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let tags: Vec<(String, String)> = tag_stmt
            .query_map(params![entry.id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        entry.tag_ids = tags.iter().map(|(id, _)| id.clone()).collect();
        entry.tag_names = tags.iter().map(|(_, name)| name.clone()).collect();
    }

    let has_more = (offset + page_size) < total_count;

    Ok(TimeEntryListResponse {
        entries,
        total_count,
        has_more,
    })
}

// ─────────────────────────────────────────────────────────────
// T025: time_entry_autocomplete
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AutocompleteRequest {
    pub query: String,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct AutocompleteSuggestion {
    pub description: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub task_id: Option<String>,
    pub task_name: Option<String>,
    pub tag_ids: Vec<String>,
    pub is_orphaned: bool,
}

#[derive(Serialize)]
pub struct AutocompleteResponse {
    pub suggestions: Vec<AutocompleteSuggestion>,
}

#[tauri::command]
pub fn time_entry_autocomplete(
    state: State<'_, AppState>,
    request: AutocompleteRequest,
) -> Result<AutocompleteResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let limit = request.limit.unwrap_or(10).clamp(1, 20);
    let search_pattern = format!("%{}%", request.query);

    // Distinct (description, project_id, task_id) from history, most recent first
    let mut stmt = conn
        .prepare(
            "SELECT te.description, te.project_id, p.name AS project_name,
                    te.task_id, t.name AS task_name
             FROM time_entries te
             LEFT JOIN projects p ON te.project_id = p.id
             LEFT JOIN tasks    t ON te.task_id     = t.id
             WHERE te.description LIKE ?1 AND te.ended_at IS NOT NULL
             GROUP BY te.description, te.project_id, te.task_id
             ORDER BY MAX(te.started_at) DESC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, Option<String>, Option<String>, Option<String>, Option<String>)> =
        stmt.query_map(params![search_pattern, limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut suggestions = Vec::new();

    for (description, project_id, project_name, task_id, task_name) in rows {
        // Orphan detection: referenced project or task no longer exists
        let project_orphaned = project_id.as_ref().map_or(false, |pid| {
            conn.query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                params![pid],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
                == 0
        });
        let task_orphaned = task_id.as_ref().map_or(false, |tid| {
            conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = ?1",
                params![tid],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
                == 0
        });
        let is_orphaned = project_orphaned || task_orphaned;

        // Get tags from most recent matching entry.
        // `project_id IS ?2` is NULL-safe equality (SQLite: NULL IS NULL → true).
        let tag_ids: Vec<String> = {
            let entry_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM time_entries
                     WHERE description = ?1
                       AND (project_id IS ?2)
                       AND (task_id IS ?3)
                       AND ended_at IS NOT NULL
                     ORDER BY started_at DESC LIMIT 1",
                    params![description, project_id, task_id],
                    |r| r.get(0),
                )
                .ok();

            if let Some(eid) = entry_id {
                let mut tag_stmt = conn
                    .prepare("SELECT tag_id FROM time_entry_tags WHERE time_entry_id = ?1")
                    .unwrap();
                tag_stmt
                    .query_map(params![eid], |r| r.get(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            }
        };

        suggestions.push(AutocompleteSuggestion {
            description,
            project_id: if project_orphaned { None } else { project_id },
            project_name: if project_orphaned { None } else { project_name },
            task_id: if task_orphaned { None } else { task_id },
            task_name: if task_orphaned { None } else { task_name },
            tag_ids,
            is_orphaned,
        });
    }

    Ok(AutocompleteResponse { suggestions })
}

// ─────────────────────────────────────────────────────────────
// T023: time_entry_create_manual
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TimeEntryCreateManualRequest {
    pub description: String,
    pub started_at: String,
    pub ended_at: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub force: Option<bool>,
}

#[derive(Serialize)]
pub struct TimeEntryCreateManualResponse {
    pub id: String,
}

#[tauri::command]
pub fn time_entry_create_manual(
    state: State<'_, AppState>,
    request: TimeEntryCreateManualRequest,
) -> Result<TimeEntryCreateManualResponse, String> {
    if request.started_at >= request.ended_at {
        return Err("invalid_time_range".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    if !request.force.unwrap_or(false) {
        let overlap_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM time_entries
             WHERE ended_at IS NOT NULL
             AND started_at < ?2
             AND ended_at > ?1",
            params![request.started_at, request.ended_at],
            |r| r.get(0),
        ).map_err(|e| e.to_string())?;

        if overlap_count > 0 {
            return Err("overlap_detected".to_string());
        }
    }

    let id = new_id();
    conn.execute(
        "INSERT INTO time_entries \
            (id, description, project_id, task_id, started_at, ended_at, is_break, device_id, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9)",
        params![
            id,
            request.description,
            request.project_id,
            request.task_id,
            request.started_at,
            request.ended_at,
            device_id,
            now,
            now,
        ],
    ).map_err(|e| format!("insert failed: {}", e))?;

    for tag_id in &request.tag_ids {
        conn.execute(
            "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
            params![id, tag_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(TimeEntryCreateManualResponse { id })
}

// ─────────────────────────────────────────────────────────────
// T024: time_entry_continue
// ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TimeEntryContinueRequest {
    pub source_entry_id: String,
}

#[tauri::command]
pub fn time_entry_continue(
    state: State<'_, AppState>,
    request: TimeEntryContinueRequest,
) -> Result<TimerStartResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    let (description, project_id, task_id) = conn.query_row(
        "SELECT description, project_id, task_id FROM time_entries WHERE id = ?1",
        params![request.source_entry_id],
        |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
        )),
    ).map_err(|_| "source_entry_not_found".to_string())?;

    let tag_ids: Vec<String> = {
        let mut tag_stmt = conn.prepare(
            "SELECT tag_id FROM time_entry_tags WHERE time_entry_id = ?1"
        ).map_err(|e| e.to_string())?;
        // Bind to local so MappedRows (borrowing tag_stmt) is fully consumed
        // before tag_stmt is dropped at block end — avoids E0597.
        let x: Vec<String> = tag_stmt
            .query_map(params![request.source_entry_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        x
    };

    let stopped_entry = stop_running_timer(&conn, &now)?;
    let (id, started_at) = insert_new_timer(
        &conn, &description, project_id, task_id, &tag_ids, &now,
    )?;

    Ok(TimerStartResponse { id, started_at, stopped_entry })
}

// ─────────────────────────────────────────────────────────────
// T030a: time_entry_update
// ─────────────────────────────────────────────────────────────

/// Deserializer for `Option<Option<T>>` — distinguishes absent (don't touch)
/// from JSON null (clear the field) from a real value (set the field).
fn deserialize_option_nullable<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Deserialize)]
pub struct TimeEntryUpdateRequest {
    pub id: String,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_nullable")]
    pub project_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_option_nullable")]
    pub task_id: Option<Option<String>>,
    pub tag_ids: Option<Vec<String>>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub force: Option<bool>,
}

#[derive(Serialize)]
pub struct TimeEntryUpdateResponse {
    pub id: String,
    pub modified_at: String,
}

#[tauri::command]
pub fn time_entry_update(
    state: State<'_, AppState>,
    request: TimeEntryUpdateRequest,
) -> Result<TimeEntryUpdateResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    let (curr_desc, curr_pid, curr_tid, curr_start, curr_end) = conn.query_row(
        "SELECT description, project_id, task_id, started_at, ended_at
         FROM time_entries WHERE id = ?1",
        params![request.id],
        |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        )),
    ).map_err(|_| "entry_not_found".to_string())?;

    let new_desc = request.description.unwrap_or(curr_desc);
    let new_pid = request.project_id.unwrap_or(curr_pid);
    let new_tid = request.task_id.unwrap_or(curr_tid);
    let new_start = request.started_at.unwrap_or(curr_start);
    let new_end = request.ended_at.or(curr_end);

    if let Some(ref end) = new_end {
        if new_start >= *end {
            return Err("invalid_time_range".to_string());
        }

        if !request.force.unwrap_or(false) {
            let overlap_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM time_entries
                 WHERE id != ?1 AND ended_at IS NOT NULL
                 AND started_at < ?3 AND ended_at > ?2",
                params![request.id, new_start, end],
                |r| r.get(0),
            ).map_err(|e| e.to_string())?;
            if overlap_count > 0 {
                return Err("overlap_detected".to_string());
            }
        }
    }

    conn.execute(
        "UPDATE time_entries SET description = ?1, project_id = ?2, task_id = ?3,
                started_at = ?4, ended_at = ?5, modified_at = ?6
         WHERE id = ?7",
        params![new_desc, new_pid, new_tid, new_start, new_end, now, request.id],
    ).map_err(|e| format!("update failed: {}", e))?;

    if let Some(tag_ids) = request.tag_ids {
        conn.execute(
            "DELETE FROM time_entry_tags WHERE time_entry_id = ?1",
            params![request.id],
        ).map_err(|e| e.to_string())?;
        for tag_id in &tag_ids {
            conn.execute(
                "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES (?1, ?2)",
                params![request.id, tag_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    Ok(TimeEntryUpdateResponse { id: request.id, modified_at: now })
}
