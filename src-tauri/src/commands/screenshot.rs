use serde::{Deserialize, Serialize};
use tauri::State;
use crate::commands::AppState;

#[derive(Debug, Deserialize)]
pub struct ScreenshotListRequest {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotItem {
    pub id: String,
    pub file_path: String,
    pub captured_at: String,
    pub window_title: String,
    pub process_name: String,
    pub trigger: String,
}

#[tauri::command]
pub fn screenshot_list(
    state: State<'_, AppState>,
    request: ScreenshotListRequest,
) -> Result<Vec<ScreenshotItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, file_path, captured_at, window_title, process_name, trigger \
         FROM screenshots WHERE captured_at >= ?1 AND captured_at <= ?2 \
         ORDER BY captured_at DESC"
    ).map_err(|e| e.to_string())?;
    let items = stmt.query_map(
        rusqlite::params![request.from, request.to],
        |row| Ok(ScreenshotItem {
            id: row.get(0)?,
            file_path: row.get(1)?,
            captured_at: row.get(2)?,
            window_title: row.get(3)?,
            process_name: row.get(4)?,
            trigger: row.get(5)?,
        })
    ).map_err(|e| e.to_string())?
     .filter_map(|r| r.ok())
     .collect();
    Ok(items)
}

#[tauri::command]
pub fn screenshot_delete_expired(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Read retention days from preferences
    let retention_days: i64 = conn.query_row(
        "SELECT screenshot_retention_days FROM user_preferences LIMIT 1",
        [], |r| r.get(0)
    ).unwrap_or(30);

    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();

    // Get paths to delete from disk
    let mut stmt = conn.prepare(
        "SELECT file_path FROM screenshots WHERE captured_at < ?1"
    ).map_err(|e| e.to_string())?;
    let paths: Vec<String> = stmt.query_map(
        rusqlite::params![cutoff],
        |row| row.get(0)
    ).map_err(|e| e.to_string())?
     .filter_map(|r| r.ok())
     .collect();

    // Delete files (ignore individual failures)
    for path in &paths {
        let _ = std::fs::remove_file(path);
    }

    // Delete DB rows
    let deleted_count = conn.execute(
        "DELETE FROM screenshots WHERE captured_at < ?1",
        rusqlite::params![cutoff],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "deleted_count": deleted_count }))
}
