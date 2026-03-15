use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

mod migrations;

/// Open or create the SQLite database at the best available portable path.
/// Applies WAL mode, foreign keys, and runs any pending migrations.
pub fn open() -> SqlResult<Connection> {
    let path = resolve_db_path();

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(e.to_string()),
            )
        })?;
    }

    let conn = Connection::open(&path)?;

    // Required pragmas — must be set before anything else
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
    ",
    )?;

    log::info!("Database opened at {:?}", path);

    migrations::run(&conn)?;

    Ok(conn)
}

fn resolve_db_path() -> PathBuf {
    // Primary: next to the executable (portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if is_writable(dir) {
                return dir.join("tracey.db");
            }
        }
    }

    // Fallback: %APPDATA%\tracey\tracey.db
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(appdata).join("tracey").join("tracey.db");
    log::warn!("exe_dir not writable, falling back to {:?}", path);
    path
}

fn is_writable(path: &std::path::Path) -> bool {
    let test = path.join(".tracey_write_test");
    match std::fs::File::create(&test) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test);
            true
        }
        Err(_) => false,
    }
}
