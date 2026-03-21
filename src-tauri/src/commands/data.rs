/// T081 backend — `data_delete_all` Tauri command
///
/// Wipes all local data tables (excluding `user_preferences`) and deletes all
/// screenshot files from disk, then recreates the screenshots directory empty.
/// Returns `{ "deleted_records": N }` where N is the total SQLite rows deleted.
///
/// File-system errors on screenshot deletion are logged as warnings but do NOT
/// abort the operation — the DB delete count is returned regardless.

use tauri::State;
use crate::commands::AppState;

/// Delete all app data rows and screenshot files.
///
/// Deletion order respects `PRAGMA foreign_keys = ON`:
/// - Junction/leaf tables first, then parent tables.
/// - Deleting `clients` cascades to `projects` → `tasks`; the explicit row
///   deletes for those tables are safe no-ops (0 rows remaining) but kept for
///   symmetry with the spec and for correctness if FK cascades are ever changed.
#[tauri::command]
pub fn data_delete_all(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    // Phase 1 — DB deletions + resolve screenshots path (all inside the lock)
    let (deleted_records, screenshots_dir) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;

        let mut count: i64 = 0;

        for sql in &[
            "DELETE FROM time_entry_tags",        // FK child of time_entries + tags
            "DELETE FROM time_entries",           // FK child of projects/tasks (ON DELETE SET NULL)
            "DELETE FROM window_activity_records",// standalone
            "DELETE FROM screenshots",            // standalone
            "DELETE FROM sync_queue",             // standalone
            "DELETE FROM clients",               // cascades: projects → tasks
            "DELETE FROM projects",              // 0 rows after clients cascade
            "DELETE FROM tasks",                 // 0 rows after projects cascade
            "DELETE FROM tags",                  // standalone (junction already cleared)
        ] {
            count += conn
                .execute(sql, [])
                .map_err(|e| format!("data_delete_all: {} — {}", sql, e))? as i64;
        }

        // Read configured screenshot storage path; fall back to {exe_dir}/screenshots/
        let storage_path: Option<String> = conn
            .query_row(
                "SELECT screenshot_storage_path FROM user_preferences LIMIT 1",
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten();

        let dir = if let Some(p) = storage_path.filter(|s| !s.is_empty()) {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .ok_or_else(|| "data_delete_all: no exe parent directory".to_string())?
                .join("screenshots")
        };

        (count, dir)
    }; // MutexGuard dropped here — file I/O happens after this point

    // Phase 2 — File-system cleanup (DB lock already released)
    if screenshots_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&screenshots_dir) {
            eprintln!(
                "[data_delete_all] Warning: failed to remove screenshots dir {:?}: {}",
                screenshots_dir, e
            );
            // Non-fatal — return DB count regardless of file deletion outcome
        }
    }
    let _ = std::fs::create_dir_all(&screenshots_dir);

    Ok(serde_json::json!({ "deleted_records": deleted_records }))
}
