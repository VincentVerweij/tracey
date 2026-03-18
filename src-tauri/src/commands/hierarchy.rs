use tauri::State;
use serde::{Deserialize, Serialize};
use rusqlite::params;
use crate::commands::AppState;
use chrono::Utc;
use ulid::Ulid;

fn new_id() -> String {
    Ulid::new().to_string()
}

/// Validates a CSS hex colour string: must be exactly `#RRGGBB`.
fn validate_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

// ─────────────────────────────────────────────────────────────
// T038: Client Commands
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ClientEntry {
    pub id: String,
    pub name: String,
    pub color: String,
    pub logo_path: Option<String>,
    pub is_archived: bool,
}

/// `client_list` — list all clients, optionally including archived ones.
#[tauri::command]
pub fn client_list(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let include_archived = include_archived.unwrap_or(false);

    // ?1 = 1 means "include archived" — when false, the OR short-circuits to is_archived = 0
    let mut stmt = conn.prepare(
        "SELECT id, name, color, logo_path, is_archived FROM clients \
         WHERE (?1 = 1 OR is_archived = 0) ORDER BY name",
    ).map_err(|e| e.to_string())?;

    let clients: Vec<ClientEntry> = stmt
        .query_map(params![include_archived as i64], |row| {
            Ok(ClientEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                logo_path: row.get(3)?,
                is_archived: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "clients": clients }))
}

#[derive(Deserialize)]
pub struct ClientCreateRequest {
    pub name: String,
    pub color: String,
    pub logo_path: Option<String>,
}

/// `client_create` — create a new client.
/// Errors: `"name_conflict"`, `"invalid_color"`, `"logo_not_found"`.
#[tauri::command]
pub fn client_create(
    state: State<'_, AppState>,
    request: ClientCreateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }
    if !validate_color(&request.color) {
        return Err("invalid_color".to_string());
    }
    if let Some(ref path) = request.logo_path {
        if !std::path::Path::new(path).exists() {
            return Err("logo_not_found".to_string());
        }
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clients WHERE name = ?1 AND is_archived = 0",
            params![name],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count > 0 {
        return Err("name_conflict".to_string());
    }

    let now = Utc::now().to_rfc3339();
    let id = new_id();

    conn.execute(
        "INSERT INTO clients (id, name, color, logo_path, is_archived, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
        params![id, name, request.color, request.logo_path, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": id }))
}

#[derive(Deserialize)]
pub struct ClientUpdateRequest {
    pub id: String,
    pub name: String,
    pub color: String,
    pub logo_path: Option<String>,
}

/// `client_update` — update name, color, logo_path for an existing client.
#[tauri::command]
pub fn client_update(
    state: State<'_, AppState>,
    request: ClientUpdateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }
    if !validate_color(&request.color) {
        return Err("invalid_color".to_string());
    }
    if let Some(ref path) = request.logo_path {
        if !std::path::Path::new(path).exists() {
            return Err("logo_not_found".to_string());
        }
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    // Check for conflict excluding this record
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clients WHERE name = ?1 AND id != ?2",
            params![name, request.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count > 0 {
        return Err("name_conflict".to_string());
    }

    conn.execute(
        "UPDATE clients SET name = ?1, color = ?2, logo_path = ?3, modified_at = ?4 \
         WHERE id = ?5",
        params![name, request.color, request.logo_path, now, request.id],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "modified_at": now }))
}

/// `client_archive` — mark client as archived.
#[tauri::command]
pub fn client_archive(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE clients SET is_archived = 1, modified_at = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "modified_at": now }))
}

/// `client_unarchive` — restore a client from archived state.
#[tauri::command]
pub fn client_unarchive(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE clients SET is_archived = 0, modified_at = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "modified_at": now }))
}

/// `client_delete` — delete client and cascade to projects/tasks. Returns orphan counts.
/// Time entries that referenced the deleted entities are retained; their project_id/task_id
/// are set to NULL (orphan retention per spec US3 acceptance scenario 6).
#[tauri::command]
pub fn client_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Count projects belonging to this client
    let deleted_projects: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE client_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Count tasks belonging to this client's projects
    let deleted_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks \
             WHERE project_id IN (SELECT id FROM projects WHERE client_id = ?1)",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Count time entries with dangling refs (project or task points into deleted scope)
    let orphaned_entries: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM time_entries \
             WHERE project_id IN (SELECT id FROM projects WHERE client_id = ?1) \
                OR task_id IN (SELECT id FROM tasks \
                               WHERE project_id IN (SELECT id FROM projects WHERE client_id = ?1))",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // NULL out project_id on affected entries
    conn.execute(
        "UPDATE time_entries SET project_id = NULL \
         WHERE project_id IN (SELECT id FROM projects WHERE client_id = ?1)",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // NULL out task_id on affected entries
    conn.execute(
        "UPDATE time_entries SET task_id = NULL \
         WHERE task_id IN (SELECT id FROM tasks \
                           WHERE project_id IN (SELECT id FROM projects WHERE client_id = ?1))",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // Delete client — ON DELETE CASCADE removes projects then tasks via FK
    conn.execute("DELETE FROM clients WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "deleted_projects": deleted_projects,
        "deleted_tasks": deleted_tasks,
        "orphaned_entries": orphaned_entries,
    }))
}

// ─────────────────────────────────────────────────────────────
// T039: Project Commands
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProjectEntry {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub is_archived: bool,
}

/// `project_list` — list projects, filtered optionally by client_id and archived state.
#[tauri::command]
pub fn project_list(
    state: State<'_, AppState>,
    client_id: Option<String>,
    include_archived: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let include_archived = include_archived.unwrap_or(false);

    // (?1 IS NULL OR client_id = ?1) — passes None as SQL NULL to skip client filter
    // (?2 = 1 OR is_archived = 0)    — when include_archived=false, only active rows
    let mut stmt = conn.prepare(
        "SELECT id, client_id, name, is_archived FROM projects \
         WHERE (?1 IS NULL OR client_id = ?1) AND (?2 = 1 OR is_archived = 0) \
         ORDER BY name",
    ).map_err(|e| e.to_string())?;

    let projects: Vec<ProjectEntry> = stmt
        .query_map(params![client_id.as_deref(), include_archived as i64], |row| {
            Ok(ProjectEntry {
                id: row.get(0)?,
                client_id: row.get(1)?,
                name: row.get(2)?,
                is_archived: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "projects": projects }))
}

#[derive(Deserialize)]
pub struct ProjectCreateRequest {
    pub client_id: String,
    pub name: String,
}

/// `project_create` — create a project under a client.
/// Errors: `"client_not_found"`, `"name_conflict"`.
#[tauri::command]
pub fn project_create(
    state: State<'_, AppState>,
    request: ProjectCreateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Verify client exists
    let client_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clients WHERE id = ?1",
            params![request.client_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if client_count == 0 {
        return Err("client_not_found".to_string());
    }

    // Check name uniqueness within the same client
    let dupe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE client_id = ?1 AND name = ?2",
            params![request.client_id, name],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if dupe > 0 {
        return Err("name_conflict".to_string());
    }

    let now = Utc::now().to_rfc3339();
    let id = new_id();

    conn.execute(
        "INSERT INTO projects (id, client_id, name, is_archived, created_at, modified_at) \
         VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        params![id, request.client_id, name, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": id }))
}

#[derive(Deserialize)]
pub struct ProjectUpdateRequest {
    pub id: String,
    pub name: String,
}

/// `project_update` — rename a project.
#[tauri::command]
pub fn project_update(
    state: State<'_, AppState>,
    request: ProjectUpdateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    // Check name conflict within same client, excluding self
    let dupe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects \
             WHERE name = ?1 AND id != ?2 \
               AND client_id = (SELECT client_id FROM projects WHERE id = ?2)",
            params![name, request.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if dupe > 0 {
        return Err("name_conflict".to_string());
    }

    conn.execute(
        "UPDATE projects SET name = ?1, modified_at = ?2 WHERE id = ?3",
        params![name, now, request.id],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "modified_at": now }))
}

/// `project_archive` — mark project as archived.
#[tauri::command]
pub fn project_archive(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE projects SET is_archived = 1, modified_at = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "modified_at": now }))
}

/// `project_unarchive` — restore a project from archived state.
#[tauri::command]
pub fn project_unarchive(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE projects SET is_archived = 0, modified_at = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "modified_at": now }))
}

/// `project_delete` — delete project; cascade deletes tasks; orphans time entries.
#[tauri::command]
pub fn project_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Count tasks that will be deleted
    let deleted_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Count time entries that will become orphaned
    let orphaned_entries: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM time_entries \
             WHERE project_id = ?1 \
                OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // NULL out task_id on entries referencing tasks under this project
    conn.execute(
        "UPDATE time_entries SET task_id = NULL \
         WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // NULL out project_id on entries referencing this project
    conn.execute(
        "UPDATE time_entries SET project_id = NULL WHERE project_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // Delete project — ON DELETE CASCADE removes tasks via FK
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "deleted_tasks": deleted_tasks, "orphaned_entries": orphaned_entries }))
}

// ─────────────────────────────────────────────────────────────
// T040: Task Commands
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TaskEntry {
    pub id: String,
    pub project_id: String,
    pub name: String,
}

/// `task_list` — list all tasks under a project.
#[tauri::command]
pub fn task_list(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, project_id, name FROM tasks WHERE project_id = ?1 ORDER BY name",
    ).map_err(|e| e.to_string())?;

    let tasks: Vec<TaskEntry> = stmt
        .query_map(params![project_id], |row| {
            Ok(TaskEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "tasks": tasks }))
}

#[derive(Deserialize)]
pub struct TaskCreateRequest {
    pub project_id: String,
    pub name: String,
}

/// `task_create` — create a task under a project.
/// Errors: `"project_not_found"`, `"name_conflict"`.
#[tauri::command]
pub fn task_create(
    state: State<'_, AppState>,
    request: TaskCreateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Verify project exists
    let project_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE id = ?1",
            params![request.project_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if project_count == 0 {
        return Err("project_not_found".to_string());
    }

    // Check name uniqueness within this project
    let dupe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND name = ?2",
            params![request.project_id, name],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if dupe > 0 {
        return Err("name_conflict".to_string());
    }

    let now = Utc::now().to_rfc3339();
    let id = new_id();

    conn.execute(
        "INSERT INTO tasks (id, project_id, name, created_at, modified_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, request.project_id, name, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": id }))
}

#[derive(Deserialize)]
pub struct TaskUpdateRequest {
    pub id: String,
    pub name: String,
}

/// `task_update` — rename a task.
#[tauri::command]
pub fn task_update(
    state: State<'_, AppState>,
    request: TaskUpdateRequest,
) -> Result<serde_json::Value, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("name_conflict".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    // Check name conflict within same project, excluding self
    let dupe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks \
             WHERE name = ?1 AND id != ?2 \
               AND project_id = (SELECT project_id FROM tasks WHERE id = ?2)",
            params![name, request.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if dupe > 0 {
        return Err("name_conflict".to_string());
    }

    conn.execute(
        "UPDATE tasks SET name = ?1, modified_at = ?2 WHERE id = ?3",
        params![name, now, request.id],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "modified_at": now }))
}

/// `task_delete` — delete a task; orphans time entries that referenced it.
#[tauri::command]
pub fn task_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // NULL out task_id on entries referencing this task
    conn.execute(
        "UPDATE time_entries SET task_id = NULL WHERE task_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(serde_json::Value::Null)
}

// ─────────────────────────────────────────────────────────────
// T053: Fuzzy Match Commands (US5 — Quick Entry)
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FuzzyProjectMatch {
    pub project_id: String,
    pub project_name: String,
    pub client_id: String,
    pub client_name: String,
    pub score: f64, // always 0.0 — C# FuzzyMatchService scores
}

/// `fuzzy_match_projects` — return non-archived projects whose name contains the query string.
/// C# FuzzyMatchService performs the real scoring; Rust returns `score: 0.0` for every entry.
#[tauri::command]
pub fn fuzzy_match_projects(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.client_id, c.name AS client_name \
             FROM projects p \
             JOIN clients c ON p.client_id = c.id \
             WHERE p.is_archived = 0 \
               AND c.is_archived = 0 \
               AND lower(p.name) LIKE lower('%' || ?1 || '%') \
             ORDER BY p.name \
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![query, limit], |row| {
            Ok(FuzzyProjectMatch {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                client_id: row.get(2)?,
                client_name: row.get(3)?,
                score: 0.0,
            })
        })
        .map_err(|e| e.to_string())?;

    let matches: Vec<FuzzyProjectMatch> = rows
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "matches": matches }))
}

#[derive(Serialize)]
pub struct FuzzyTaskMatch {
    pub task_id: String,
    pub task_name: String,
    pub score: f64, // always 0.0 — C# FuzzyMatchService scores
}

/// `fuzzy_match_tasks` — return tasks for a project whose name contains the query string.
/// If query is empty/whitespace, all tasks up to `limit` are returned.
/// C# FuzzyMatchService performs the real scoring; Rust returns `score: 0.0` for every entry.
#[tauri::command]
pub fn fuzzy_match_tasks(
    state: State<'_, AppState>,
    project_id: String,
    query: String,
    limit: i64,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let trimmed = query.trim();

    let matches: Vec<FuzzyTaskMatch> = if trimmed.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT id, name FROM tasks \
                 WHERE project_id = ?1 \
                 ORDER BY name \
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![project_id, limit], |row| {
                Ok(FuzzyTaskMatch {
                    task_id: row.get(0)?,
                    task_name: row.get(1)?,
                    score: 0.0,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, name FROM tasks \
                 WHERE project_id = ?1 \
                   AND lower(name) LIKE lower('%' || ?2 || '%') \
                 ORDER BY name \
                 LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![project_id, trimmed, limit], |row| {
                Ok(FuzzyTaskMatch {
                    task_id: row.get(0)?,
                    task_name: row.get(1)?,
                    score: 0.0,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    Ok(serde_json::json!({ "matches": matches }))
}
