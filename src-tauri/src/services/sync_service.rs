//! T071 — External DB schema migration runner (Postgres DDL from contracts/sync-api.md)
//! T072 — SyncService background loop: upsert batches, 30-second interval, immediate trigger
//! T073 — Offline resilience: queue persisted in SQLite; auto-replay on reconnect

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_postgres::NoTls;

use crate::commands::AppState;
use crate::commands::sync::read_keychain_uri;

// ─────────────────────────────────────────────────────────────
// T071 — External Postgres schema migration
// ─────────────────────────────────────────────────────────────

/// Schema version applied to the external Postgres database.
const EXTERNAL_SCHEMA_VERSION: i32 = 1;

/// Full DDL for the external Postgres schema (sync-api.md schema version 1).
/// Applied idempotently via IF NOT EXISTS / DO NOTHING guards.
/// Name-based UNIQUE constraints are intentionally absent: multi-device sync means two
/// devices can independently create a "Client A" before sync starts. The PK (id) is the
/// real uniqueness anchor.
const EXTERNAL_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS clients (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,
    logo_path   TEXT,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    device_id   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    client_id   TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    device_id   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    device_id   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    device_id   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

-- Drop legacy name-unique constraints that would block multi-device upserts.
-- IF EXISTS makes these safe to rerun on databases that never had the constraints.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'clients_name_unique') THEN
        ALTER TABLE clients DROP CONSTRAINT clients_name_unique;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'projects_client_name_unique') THEN
        ALTER TABLE projects DROP CONSTRAINT projects_client_name_unique;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'tags_name_unique') THEN
        ALTER TABLE tags DROP CONSTRAINT tags_name_unique;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS time_entries (
    id          TEXT PRIMARY KEY,
    description TEXT NOT NULL DEFAULT '',
    started_at  TIMESTAMPTZ NOT NULL,
    ended_at    TIMESTAMPTZ,
    project_id  TEXT REFERENCES projects(id) ON DELETE SET NULL,
    task_id     TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    is_break    BOOLEAN NOT NULL DEFAULT false,
    device_id   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_time_entries_running'
    ) THEN
        CREATE UNIQUE INDEX idx_time_entries_running
            ON time_entries (device_id) WHERE ended_at IS NULL;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS time_entry_tags (
    time_entry_id TEXT NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    tag_id        TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (time_entry_id, tag_id)
);

CREATE TABLE IF NOT EXISTS window_activity_records (
    id             TEXT PRIMARY KEY,
    process_name   TEXT NOT NULL,
    window_title   TEXT NOT NULL,
    window_handle  TEXT NOT NULL,
    recorded_at    TIMESTAMPTZ NOT NULL,
    device_id      TEXT NOT NULL,
    synced_at      TIMESTAMPTZ
);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_war_device_recorded'
    ) THEN
        CREATE INDEX idx_war_device_recorded
            ON window_activity_records (device_id, recorded_at DESC);
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS user_preferences (
    device_id                           TEXT PRIMARY KEY,
    local_timezone                      TEXT NOT NULL DEFAULT 'UTC',
    inactivity_timeout_seconds          INTEGER NOT NULL DEFAULT 300,
    screenshot_interval_seconds         INTEGER NOT NULL DEFAULT 60,
    screenshot_retention_days           INTEGER NOT NULL DEFAULT 30,
    screenshot_storage_path             TEXT,
    timer_notification_threshold_hours  DOUBLE PRECISION NOT NULL DEFAULT 8.0,
    page_size                           INTEGER NOT NULL DEFAULT 50,
    external_db_enabled                 BOOLEAN NOT NULL DEFAULT false,
    notification_channels_json          TEXT,
    process_deny_list_json              TEXT NOT NULL
        DEFAULT '["keepass","1password","bitwarden","lastpass"]',
    modified_at                         TIMESTAMPTZ NOT NULL
);
"#;

/// Connect to Postgres, apply the external schema migrations, and disconnect.
/// Called by `sync_configure` to validate the connection URI before storing it.
pub async fn connect_and_migrate(uri: &str) -> Result<(), String> {
    let (client, connection) = tokio_postgres::connect(uri, NoTls)
        .await
        .map_err(|e| format!("postgres connect failed: {e}"))?;

    // Drive the connection in a separate task
    tauri::async_runtime::spawn(async move {
        if let Err(e) = connection.await {
            log::error!("[sync_service] postgres connection error: {}", e);
        }
    });

    apply_migrations(&client).await?;
    Ok(())
}

async fn apply_migrations(client: &tokio_postgres::Client) -> Result<(), String> {
    // Check whether our schema version is already applied
    client
        .batch_execute(EXTERNAL_DDL)
        .await
        .map_err(|e| format!("external DDL failed: {e}"))?;

    // Record in schema_migrations (idempotent)
    client
        .execute(
            "INSERT INTO schema_migrations (version) VALUES ($1) ON CONFLICT (version) DO NOTHING",
            &[&EXTERNAL_SCHEMA_VERSION],
        )
        .await
        .map_err(|e| format!("schema_migrations insert failed: {e}"))?;

    log::info!("[sync_service] External schema migrations applied (version {})", EXTERNAL_SCHEMA_VERSION);
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// T072 / T073 — Sync cycle (shared between background loop and sync_trigger command)
// ─────────────────────────────────────────────────────────────

/// Collect sync work from SQLite, upsert to Postgres, return (synced_records, errors, new_cursor).
/// The `cursor` is the last `modified_at` timestamp successfully synced; None = sync everything.
///
/// Upserts are done via `modified_at` scan (belt) + sync_queue delete entries (suspenders).
pub async fn run_sync_cycle_inline(
    db: &std::sync::Mutex<rusqlite::Connection>,
    uri: &str,
    cursor: Option<String>,
) -> Result<(i64, i64, String), String> {
    let (pg_client, connection) = tokio_postgres::connect(uri, NoTls)
        .await
        .map_err(|e| format!("postgres connect: {e}"))?;

    tauri::async_runtime::spawn(async move {
        if let Err(e) = connection.await {
            log::error!("[sync_service] bg connection error: {}", e);
        }
    });

    // Always run the DDL so schema is current (idempotent — safe to run on every cycle).
    // This drops legacy name-unique constraints that would otherwise block upserts.
    pg_client
        .batch_execute(EXTERNAL_DDL)
        .await
        .map_err(|e| format!("schema refresh failed: {e}"))?;

    // Use "" as the lower bound: any non-empty modified_at string is >= "",
    // so cursor=None results in a full unconditional table scan.
    let cursor_ts = cursor.unwrap_or_default();
    let mut synced: i64 = 0;
    let mut errors: i64 = 0;
    // Capture the first distinct error message to make root-cause obvious in logs.
    let mut first_error: Option<String> = None;
    let mut record_first_error = |context: &str, e: &tokio_postgres::Error| {
        if first_error.is_none() {
            first_error = Some(format!("{context}: {e}"));
        }
    };

    // ── Upsert clients ────────────────────────────────────────────────────
    let clients = read_clients(db, &cursor_ts)?;
    let (mut c_ok, mut c_err) = (0i64, 0i64);
    for row in &clients {
        let r = pg_client
            .execute(
                "INSERT INTO clients (id, name, color, logo_path, is_archived, device_id, created_at, modified_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7::text::timestamptz,$8::text::timestamptz)
                 ON CONFLICT (id) DO UPDATE SET
                   name        = EXCLUDED.name,
                   color       = EXCLUDED.color,
                   is_archived = EXCLUDED.is_archived,
                   device_id   = EXCLUDED.device_id,
                   modified_at = GREATEST(clients.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= clients.modified_at",
                &[&row.id, &row.name, &row.color, &row.logo_path,
                  &row.is_archived, &row.device_id, &row.created_at, &row.modified_at],
            )
            .await;
        match r {
            Ok(_) => { synced += 1; c_ok += 1; }
            Err(e) => { record_first_error(&format!("clients/{}", row.id), &e); log::error!("[sync] clients/{}: {}", row.id, e); errors += 1; c_err += 1; }
        }
    }
    if c_err > 0 { log::error!("[sync] clients: {c_ok} ok, {c_err} failed"); }

    // ── Upsert projects ────────────────────────────────────────────────────
    let projects = read_projects(db, &cursor_ts)?;
    let (mut p_ok, mut p_err) = (0i64, 0i64);
    for row in &projects {
        let r = pg_client
            .execute(
                "INSERT INTO projects (id, client_id, name, is_archived, device_id, created_at, modified_at)
                 VALUES ($1,$2,$3,$4,$5,$6::text::timestamptz,$7::text::timestamptz)
                 ON CONFLICT (id) DO UPDATE SET
                   client_id   = EXCLUDED.client_id,
                   name        = EXCLUDED.name,
                   is_archived = EXCLUDED.is_archived,
                   device_id   = EXCLUDED.device_id,
                   modified_at = GREATEST(projects.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= projects.modified_at",
                &[&row.id, &row.client_id, &row.name, &row.is_archived,
                  &row.device_id, &row.created_at, &row.modified_at],
            )
            .await;
        match r {
            Ok(_) => { synced += 1; p_ok += 1; }
            Err(e) => { record_first_error(&format!("projects/{}", row.id), &e); log::error!("[sync] projects/{}: {}", row.id, e); errors += 1; p_err += 1; }
        }
    }
    if p_err > 0 { log::error!("[sync] projects: {p_ok} ok, {p_err} failed"); }

    // ── Upsert tasks ───────────────────────────────────────────────────────
    let tasks = read_tasks(db, &cursor_ts)?;
    let (mut t_ok, mut t_err) = (0i64, 0i64);
    for row in &tasks {
        let r = pg_client
            .execute(
                "INSERT INTO tasks (id, project_id, name, device_id, created_at, modified_at)
                 VALUES ($1,$2,$3,$4,$5::text::timestamptz,$6::text::timestamptz)
                 ON CONFLICT (id) DO UPDATE SET
                   project_id  = EXCLUDED.project_id,
                   name        = EXCLUDED.name,
                   device_id   = EXCLUDED.device_id,
                   modified_at = GREATEST(tasks.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= tasks.modified_at",
                &[&row.id, &row.project_id, &row.name, &row.device_id,
                  &row.created_at, &row.modified_at],
            )
            .await;
        match r {
            Ok(_) => { synced += 1; t_ok += 1; }
            Err(e) => { record_first_error(&format!("tasks/{}", row.id), &e); log::error!("[sync] tasks/{}: {}", row.id, e); errors += 1; t_err += 1; }
        }
    }
    if t_err > 0 { log::error!("[sync] tasks: {t_ok} ok, {t_err} failed"); }

    // ── Upsert tags ────────────────────────────────────────────────────────
    let tags = read_tags(db, &cursor_ts)?;
    let (mut g_ok, mut g_err) = (0i64, 0i64);
    for row in &tags {
        let r = pg_client
            .execute(
                "INSERT INTO tags (id, name, device_id, created_at, modified_at)
                 VALUES ($1,$2,$3,$4::text::timestamptz,$5::text::timestamptz)
                 ON CONFLICT (id) DO UPDATE SET
                   name        = EXCLUDED.name,
                   device_id   = EXCLUDED.device_id,
                   modified_at = GREATEST(tags.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= tags.modified_at",
                &[&row.id, &row.name, &row.device_id, &row.created_at, &row.modified_at],
            )
            .await;
        match r {
            Ok(_) => { synced += 1; g_ok += 1; }
            Err(e) => { record_first_error(&format!("tags/{}", row.id), &e); log::error!("[sync] tags/{}: {}", row.id, e); errors += 1; g_err += 1; }
        }
    }
    if g_err > 0 { log::error!("[sync] tags: {g_ok} ok, {g_err} failed"); }

    // ── Upsert time_entries (batched, up to 50 per cycle) ─────────────────
    let entries = read_time_entries(db, &cursor_ts, 50)?;
    let (mut e_ok, mut e_err) = (0i64, 0i64);
    for row in &entries {
        let r = pg_client
            .execute(
                "INSERT INTO time_entries
                   (id, description, started_at, ended_at, project_id, task_id, is_break, device_id, created_at, modified_at)
                 VALUES ($1,$2,$3::text::timestamptz,$4::text::timestamptz,$5,$6,$7,$8,$9::text::timestamptz,$10::text::timestamptz)
                 ON CONFLICT (id) DO UPDATE SET
                   description = EXCLUDED.description,
                   started_at  = EXCLUDED.started_at,
                   ended_at    = EXCLUDED.ended_at,
                   project_id  = EXCLUDED.project_id,
                   task_id     = EXCLUDED.task_id,
                   is_break    = EXCLUDED.is_break,
                   device_id   = EXCLUDED.device_id,
                   modified_at = GREATEST(time_entries.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= time_entries.modified_at",
                &[
                    &row.id, &row.description,
                    &row.started_at, &row.ended_at,
                    &row.project_id, &row.task_id, &row.is_break,
                    &row.device_id, &row.created_at, &row.modified_at,
                ],
            )
            .await;
        match r {
            Ok(_) => {
                sync_entry_tags(&pg_client, db, &row.id).await;
                synced += 1;
                e_ok += 1;
            }
            Err(e) => { record_first_error(&format!("time_entries/{}", row.id), &e); log::error!("[sync] time_entries/{}: {}", row.id, e); errors += 1; e_err += 1; }
        }
    }
    if e_err > 0 { log::error!("[sync] time_entries: {e_ok} ok, {e_err} failed"); }

    // ── Sync window_activity_records (up to 500 per cycle) ────────────────
    let war_rows = read_window_activity(db, 500)?;
    let war_ids: Vec<String> = war_rows.iter().map(|r| r.id.clone()).collect();
    for row in &war_rows {
        let r = pg_client
            .execute(
                "INSERT INTO window_activity_records
                   (id, process_name, window_title, window_handle, recorded_at, device_id)
                 VALUES ($1,$2,$3,$4,$5::text::timestamptz,$6)
                 ON CONFLICT (id) DO NOTHING",
                &[&row.id, &row.process_name, &row.window_title,
                  &row.window_handle, &row.recorded_at, &row.device_id],
            )
            .await;
        match r { Ok(_) => synced += 1, Err(e) => { log::error!("[sync] window_activity/{}: {}", row.id, e); errors += 1; } }
    }
    // Mark synced window activity records
    if !war_ids.is_empty() {
        let now = Utc::now().to_rfc3339();
        let conn = db.lock().map_err(|e| e.to_string())?;
        for id in &war_ids {
            let _ = conn.execute(
                "UPDATE window_activity_records SET synced_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            );
        }
    }

    // ── Sync user_preferences ──────────────────────────────────────────────
    if let Ok(prefs) = read_user_preferences(db) {
        let modified_at = Utc::now().to_rfc3339();
        let device_id = device_id();
        let r = pg_client
            .execute(
                "INSERT INTO user_preferences
                   (device_id, local_timezone, inactivity_timeout_seconds,
                    screenshot_interval_seconds, screenshot_retention_days,
                    timer_notification_threshold_hours, page_size,
                    external_db_enabled, notification_channels_json,
                    process_deny_list_json, modified_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::text::timestamptz)
                 ON CONFLICT (device_id) DO UPDATE SET
                   local_timezone                     = EXCLUDED.local_timezone,
                   inactivity_timeout_seconds         = EXCLUDED.inactivity_timeout_seconds,
                   screenshot_interval_seconds        = EXCLUDED.screenshot_interval_seconds,
                   screenshot_retention_days          = EXCLUDED.screenshot_retention_days,
                   timer_notification_threshold_hours = EXCLUDED.timer_notification_threshold_hours,
                   page_size                          = EXCLUDED.page_size,
                   external_db_enabled                = EXCLUDED.external_db_enabled,
                   notification_channels_json         = EXCLUDED.notification_channels_json,
                   process_deny_list_json             = EXCLUDED.process_deny_list_json,
                   modified_at                        = GREATEST(user_preferences.modified_at, EXCLUDED.modified_at)
                 WHERE EXCLUDED.modified_at >= user_preferences.modified_at",
                &[
                    &device_id,
                    &prefs.local_timezone,
                    &(prefs.inactivity_timeout_seconds as i32),
                    &(prefs.screenshot_interval_seconds as i32),
                    &(prefs.screenshot_retention_days as i32),
                    &prefs.timer_notification_threshold_hours,
                    &(prefs.page_size as i32),
                    &prefs.external_db_enabled,
                    &prefs.notification_channels_json,
                    &prefs.process_deny_list_json,
                    &modified_at,
                ],
            )
            .await;
        match r { Ok(_) => synced += 1, Err(e) => { record_first_error("user_preferences", &e); log::error!("[sync] user_preferences: {}", e); errors += 1; } }
    }

    // ── Process sync_queue DELETE operations ──────────────────────────────
    let deletes = read_pending_deletes(db)?;
    let mut processed_delete_ids: Vec<i64> = vec![];
    for entry in &deletes {
        let del_result = match entry.table_name.as_str() {
            "clients" => {
                pg_client
                    .execute("DELETE FROM clients WHERE id = $1", &[&entry.record_id])
                    .await
            }
            "tags" => {
                pg_client
                    .execute("DELETE FROM tags WHERE id = $1", &[&entry.record_id])
                    .await
            }
            "projects" => {
                pg_client
                    .execute("DELETE FROM projects WHERE id = $1", &[&entry.record_id])
                    .await
            }
            "tasks" => {
                pg_client
                    .execute("DELETE FROM tasks WHERE id = $1", &[&entry.record_id])
                    .await
            }
            _ => {
                log::warn!("[sync] unknown table in delete queue: {}", entry.table_name);
                processed_delete_ids.push(entry.id);
                continue;
            }
        };
        match del_result {
            Ok(_) => {
                synced += 1;
                processed_delete_ids.push(entry.id);
            }
            Err(e) => {
                errors += 1;
                // Increment attempts counter for backoff
                let conn = db.lock().map_err(|e| e.to_string())?;
                let _ = conn.execute(
                    "UPDATE sync_queue SET attempts = attempts + 1 WHERE id = ?1",
                    rusqlite::params![entry.id],
                );
                log::error!("[sync] delete queue {}/{}: {}", entry.table_name, entry.record_id, e);
            }
        }
    }

    // Remove successfully processed delete entries from sync_queue
    if !processed_delete_ids.is_empty() {
        let conn = db.lock().map_err(|e| e.to_string())?;
        for id in &processed_delete_ids {
            let _ = conn.execute("DELETE FROM sync_queue WHERE id = ?1", rusqlite::params![id]);
        }
    }

    // Advance cursor to now so next cycle only picks up new writes
    let new_cursor = Utc::now().to_rfc3339();
    if errors == 0 {
        log::info!("[sync] cycle complete — synced={synced}");
    } else {
        log::error!("[sync] cycle complete — synced={synced} errors={errors}");
        if let Some(ref root) = first_error {
            log::error!("[sync] first error: {root}");
        }
    }
    Ok((synced, errors, new_cursor))
}

// ─────────────────────────────────────────────────────────────
// T072 — Background sync loop (30-second interval + notify trigger)
// ─────────────────────────────────────────────────────────────

/// Start the background sync loop. Runs indefinitely. Woken by `sync_notify` or every 30 s.
pub fn start_sync_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        log::info!("[sync_service] Background sync loop started");

        // ── Startup: restore URI cache from keychain if sync was previously enabled ──
        {
            let app_state = app.state::<AppState>();
            let sync_enabled = {
                match app_state.db.lock() {
                    Ok(conn) => conn
                        .query_row("SELECT external_db_enabled FROM user_preferences LIMIT 1", [], |r| r.get::<_, bool>(0))
                        .unwrap_or(false),
                    Err(_) => false,
                }
            };
            if sync_enabled {
                match read_keychain_uri() {
                    Ok(uri) => {
                        let mut ss = app_state.sync_state.lock().unwrap_or_else(|e| e.into_inner());
                        ss.cached_uri = Some(uri);
                        log::info!("[sync_service] URI restored from keychain into cache");
                    }
                    Err(e) => {
                        log::warn!("[sync_service] Sync enabled but keychain restore failed: {e}");
                    }
                }
            }
        }

        loop {
            let app_state = app.state::<AppState>();

            // Wait for trigger or 30-second interval (T072: "30-second interval; immediate on local write")
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {}
                _ = app_state.sync_notify.notified() => {
                    log::debug!("[sync_service] Woken by sync_notify");
                }
            }

            // Check if sync is enabled and grab the cached URI (no keychain read in the hot path)
            let enabled = {
                let conn = match app_state.db.lock() {
                    Ok(c) => c,
                    Err(e) => { log::error!("[sync_service] DB lock failed: {e}"); continue; }
                };
                conn.query_row("SELECT external_db_enabled FROM user_preferences LIMIT 1", [], |r| r.get::<_, bool>(0))
                    .unwrap_or(false)
            };

            if !enabled {
                continue;
            }

            // Read URI from in-memory cache (populated by sync_configure or startup restore)
            let uri = {
                let ss = app_state.sync_state.lock().unwrap_or_else(|e| e.into_inner());
                ss.cached_uri.clone()
            };

            let uri = match uri {
                Some(u) => u,
                None => {
                    log::warn!("[sync_service] Sync enabled but no URI cached — skipping cycle");
                    continue;
                }
            };

            // Get current cursor (don't hold lock across await)
            let cursor = {
                let ss = app_state.sync_state.lock().unwrap_or_else(|e| e.into_inner());
                ss.last_sync_cursor.clone()
            };

            match run_sync_cycle_inline(&app_state.db, &uri, cursor).await {
                Ok((synced, errors, new_cursor)) => {
                    let now = Utc::now().to_rfc3339();
                    {
                        let mut ss = app_state.sync_state.lock().unwrap_or_else(|e| e.into_inner());
                        ss.connected = true;
                        ss.last_sync_at = Some(now.clone());
                        ss.last_sync_cursor = Some(new_cursor);
                        ss.last_error = None;
                    }
                    let _ = app.emit("tracey://sync-status-changed", serde_json::json!({
                        "connected": true,
                        "last_sync_at": now,
                        "synced_records": synced,
                        "errors": errors
                    }));
                }
                Err(e) => {
                    log::error!("[sync_service] Sync cycle failed: {}", e);
                    {
                        let mut ss = app_state.sync_state.lock().unwrap_or_else(|e2| e2.into_inner());
                        ss.connected = false;
                        ss.last_error = Some(e.clone());
                    }
                    let _ = app.emit("tracey://sync-status-changed", serde_json::json!({
                        "connected": false,
                        "last_error": e
                    }));
                }
            }
        }
    });
}

// ─────────────────────────────────────────────────────────────
// SQLite read helpers (used by sync cycle)
// ─────────────────────────────────────────────────────────────

struct ClientRow { id: String, name: String, color: String, logo_path: Option<String>, is_archived: bool, device_id: String, created_at: String, modified_at: String }
struct ProjectRow { id: String, client_id: String, name: String, is_archived: bool, device_id: String, created_at: String, modified_at: String }
struct TaskRow { id: String, project_id: String, name: String, device_id: String, created_at: String, modified_at: String }
struct TagRow { id: String, name: String, device_id: String, created_at: String, modified_at: String }
struct TimeEntryRow {
    id: String, description: String, started_at: String, ended_at: Option<String>,
    project_id: Option<String>, task_id: Option<String>, is_break: bool,
    device_id: String, created_at: String, modified_at: String,
}
struct WarRow { id: String, process_name: String, window_title: String, window_handle: String, recorded_at: String, device_id: String }
struct DeleteQueueEntry { id: i64, table_name: String, record_id: String }

fn read_clients(db: &std::sync::Mutex<rusqlite::Connection>, cursor: &str) -> Result<Vec<ClientRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, name, color, logo_path, is_archived, device_id, created_at, modified_at
         FROM clients WHERE modified_at >= ?1 ORDER BY modified_at"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![cursor], |r| {
        let raw_did: String = r.get(5)?;
        Ok(ClientRow {
            id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, logo_path: r.get(3)?,
            is_archived: r.get::<_,i64>(4)? != 0,
            device_id: if raw_did.is_empty() { device_id() } else { raw_did },
            created_at: r.get(6)?, modified_at: r.get(7)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_projects(db: &std::sync::Mutex<rusqlite::Connection>, cursor: &str) -> Result<Vec<ProjectRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, client_id, name, is_archived, device_id, created_at, modified_at
         FROM projects WHERE modified_at >= ?1 ORDER BY modified_at"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![cursor], |r| {
        let raw_did: String = r.get(4)?;
        Ok(ProjectRow {
            id: r.get(0)?, client_id: r.get(1)?, name: r.get(2)?,
            is_archived: r.get::<_,i64>(3)? != 0,
            device_id: if raw_did.is_empty() { device_id() } else { raw_did },
            created_at: r.get(5)?, modified_at: r.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_tasks(db: &std::sync::Mutex<rusqlite::Connection>, cursor: &str) -> Result<Vec<TaskRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, device_id, created_at, modified_at
         FROM tasks WHERE modified_at >= ?1 ORDER BY modified_at"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![cursor], |r| {
        let raw_did: String = r.get(3)?;
        Ok(TaskRow {
            id: r.get(0)?, project_id: r.get(1)?, name: r.get(2)?,
            device_id: if raw_did.is_empty() { device_id() } else { raw_did },
            created_at: r.get(4)?, modified_at: r.get(5)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_tags(db: &std::sync::Mutex<rusqlite::Connection>, cursor: &str) -> Result<Vec<TagRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, name, device_id, created_at, modified_at
         FROM tags WHERE modified_at >= ?1 ORDER BY modified_at"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![cursor], |r| {
        let raw_did: String = r.get(2)?;
        Ok(TagRow {
            id: r.get(0)?, name: r.get(1)?,
            device_id: if raw_did.is_empty() { device_id() } else { raw_did },
            created_at: r.get(3)?, modified_at: r.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_time_entries(
    db: &std::sync::Mutex<rusqlite::Connection>,
    cursor: &str,
    limit: i64,
) -> Result<Vec<TimeEntryRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, description, started_at, ended_at, project_id, task_id, is_break, device_id, created_at, modified_at
         FROM time_entries WHERE modified_at >= ?1 ORDER BY modified_at LIMIT ?2"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![cursor, limit], |r| Ok(TimeEntryRow {
        id: r.get(0)?, description: r.get(1)?, started_at: r.get(2)?,
        ended_at: r.get(3)?, project_id: r.get(4)?, task_id: r.get(5)?,
        is_break: r.get::<_,i64>(6)? != 0, device_id: r.get(7)?,
        created_at: r.get(8)?, modified_at: r.get(9)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_window_activity(
    db: &std::sync::Mutex<rusqlite::Connection>,
    limit: i64,
) -> Result<Vec<WarRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, process_name, window_title, window_handle, recorded_at, device_id
         FROM window_activity_records WHERE synced_at IS NULL ORDER BY recorded_at LIMIT ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![limit], |r| Ok(WarRow {
        id: r.get(0)?, process_name: r.get(1)?, window_title: r.get(2)?,
        window_handle: r.get(3)?, recorded_at: r.get(4)?, device_id: r.get(5)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_user_preferences(db: &std::sync::Mutex<rusqlite::Connection>) -> Result<crate::models::UserPreferences, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, local_timezone, inactivity_timeout_seconds,
                screenshot_interval_seconds, screenshot_retention_days,
                screenshot_storage_path, timer_notification_threshold_hours,
                page_size, external_db_uri_stored, external_db_enabled,
                notification_channels_json, process_deny_list_json
         FROM user_preferences LIMIT 1",
        [],
        |row| Ok(crate::models::UserPreferences {
            id: row.get(0)?, local_timezone: row.get(1)?,
            inactivity_timeout_seconds: row.get(2)?,
            screenshot_interval_seconds: row.get(3)?, screenshot_retention_days: row.get(4)?,
            screenshot_storage_path: row.get(5)?, timer_notification_threshold_hours: row.get(6)?,
            page_size: row.get(7)?, external_db_uri_stored: row.get(8)?,
            external_db_enabled: row.get(9)?, notification_channels_json: row.get(10)?,
            process_deny_list_json: row.get(11)?,
        }),
    )
    .map_err(|e| e.to_string())
}

fn read_pending_deletes(db: &std::sync::Mutex<rusqlite::Connection>) -> Result<Vec<DeleteQueueEntry>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, table_name, record_id FROM sync_queue
         WHERE operation = 'delete' AND attempts < 5
         ORDER BY queued_at LIMIT 100",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(DeleteQueueEntry {
        id: r.get(0)?, table_name: r.get(1)?, record_id: r.get(2)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Sync time_entry_tags junction table for a single entry.
/// Delete all existing associations in Postgres for this entry, then re-insert from SQLite.
async fn sync_entry_tags(
    pg: &tokio_postgres::Client,
    db: &std::sync::Mutex<rusqlite::Connection>,
    entry_id: &str,
) {
    // Read current tags from SQLite — collect into Vec so conn/stmt are dropped before await
    let tag_ids: Vec<String> = {
        let conn = match db.lock() { Ok(c) => c, Err(_) => return };
        let mut stmt = match conn.prepare(
            "SELECT tag_id FROM time_entry_tags WHERE time_entry_id = ?1") { Ok(s) => s, Err(_) => return };
        let collected: Vec<String> = stmt
            .query_map(rusqlite::params![entry_id], |r| r.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        collected
    }; // conn and stmt dropped here — safe to await below

    // Delete existing associations in Postgres
    let _ = pg.execute("DELETE FROM time_entry_tags WHERE time_entry_id = $1", &[&entry_id]).await;

    // Re-insert current associations
    for tag_id in &tag_ids {
        let _ = pg.execute(
            "INSERT INTO time_entry_tags (time_entry_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            &[&entry_id, tag_id],
        ).await;
    }
}

/// Returns the device identifier used for `device_id` columns.
fn device_id() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local".to_string())
}
