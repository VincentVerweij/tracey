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
    seed_first_launch(&conn, &path)?;

    Ok(conn)
}

/// Resolve the best-available portable database path.
/// `exe_override` is used in tests to inject a fake executable path.
pub fn resolve_db_path_for(exe_override: Option<&std::path::Path>) -> PathBuf {
    let candidate = exe_override
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_exe().ok())
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(dir) = candidate {
        if is_writable(&dir) {
            return dir.join("tracey.db");
        }
    }

    // Fallback: %APPDATA%\tracey\tracey.db
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(appdata).join("tracey").join("tracey.db");
    log::warn!("exe_dir not writable, falling back to {:?}", path);
    path
}

fn resolve_db_path() -> PathBuf {
    resolve_db_path_for(None)
}

pub fn is_writable(path: &std::path::Path) -> bool {
    let test = path.join(".tracey_write_test");
    match std::fs::File::create(&test) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test);
            true
        }
        Err(_) => false,
    }
}

/// Called once after migrations. Seeds the default `user_preferences` row and
/// creates the screenshots directory on first launch. No-op if already seeded.
fn seed_first_launch(conn: &Connection, db_path: &PathBuf) -> SqlResult<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM user_preferences",
        [],
        |r| r.get(0),
    )?;

    if count > 0 {
        return Ok(()); // Already seeded
    }

    log::info!("First launch detected — seeding defaults");

    // Create screenshots directory next to the DB file
    if let Some(parent) = db_path.parent() {
        let screenshots_dir = parent.join("screenshots");
        if let Err(e) = std::fs::create_dir_all(&screenshots_dir) {
            log::warn!("Could not create screenshots dir: {}", e);
            // Non-fatal — screenshot capture will fail gracefully later
        } else {
            log::info!("Created screenshots directory at {:?}", screenshots_dir);
        }
    }

    // Column names verified against 001_initial_schema.sql:
    // id, local_timezone, inactivity_timeout_seconds, screenshot_interval_seconds,
    // screenshot_retention_days, screenshot_storage_path,
    // timer_notification_threshold_hours, page_size, external_db_uri_stored,
    // external_db_enabled, notification_channels_json, process_deny_list_json
    conn.execute(
        "INSERT INTO user_preferences (
            id,
            inactivity_timeout_seconds,
            screenshot_interval_seconds,
            screenshot_retention_days,
            screenshot_storage_path,
            local_timezone,
            page_size,
            process_deny_list_json,
            external_db_uri_stored,
            external_db_enabled,
            notification_channels_json,
            timer_notification_threshold_hours
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            1i64,           // id = 1 (singleton enforced by CHECK (id = 1))
            300i64,         // inactivity_timeout_seconds: 5 minutes
            300i64,         // screenshot_interval_seconds: 5 minutes
            30i64,          // screenshot_retention_days: 30 days
            None::<String>, // screenshot_storage_path: NULL → runtime default {exe_dir}/screenshots/
            "UTC",          // local_timezone
            25i64,          // page_size
            r#"["keepass","1password","bitwarden","lastpass"]"#, // process_deny_list_json
            0i64,           // external_db_uri_stored: false
            0i64,           // external_db_enabled: false
            None::<String>, // notification_channels_json: NULL
            8.0f64,         // timer_notification_threshold_hours
        ],
    )?;

    log::info!("Default user_preferences seeded");
    Ok(())
}
