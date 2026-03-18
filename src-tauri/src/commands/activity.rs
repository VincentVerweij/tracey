use tauri::State;
use serde::Serialize;
use rusqlite::params;
use crate::commands::AppState;
use chrono::Utc;
use ulid::Ulid;

fn new_id() -> String {
    Ulid::new().to_string()
}

// ─────────────────────────────────────────────────────────────
// T058: Tag Commands
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TagEntry {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub entry_count: i64,
}

/// `tag_list` — list all tags ordered by name, with entry_count showing how many
/// time entries currently use each tag via the time_entry_tags join table.
#[tauri::command]
pub fn tag_list(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.created_at, COUNT(tet.tag_id) AS entry_count \
             FROM tags t \
             LEFT JOIN time_entry_tags tet ON tet.tag_id = t.id \
             GROUP BY t.id \
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;

    let tags: Vec<TagEntry> = stmt
        .query_map([], |row| {
            Ok(TagEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                entry_count: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "tags": tags }))
}

/// `tag_create` — create a new tag.
/// Errors: `"name_required"` (empty name), `"name_conflict"` (duplicate).
#[tauri::command]
pub fn tag_create(state: State<'_, AppState>, name: String) -> Result<serde_json::Value, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("name_required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tags WHERE name = ?1",
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
        "INSERT INTO tags (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, now, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": id }))
}

/// `tag_delete` — delete a tag by ID.  
/// The `time_entry_tags` rows are removed by the `ON DELETE CASCADE` constraint —
/// linked time entries themselves are preserved.  
/// Returns `affected_entries`: the number of entries that had this tag removed.  
/// Errors: `"not_found"`.
#[tauri::command]
pub fn tag_delete(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Count affected entries BEFORE deletion so we can report the number accurately.
    let affected: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM time_entry_tags WHERE tag_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let deleted = conn
        .execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    if deleted == 0 {
        return Err("not_found".to_string());
    }

    Ok(serde_json::json!({ "affected_entries": affected }))
}
