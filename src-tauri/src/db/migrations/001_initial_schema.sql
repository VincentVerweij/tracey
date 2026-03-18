-- Migration 001: Initial Schema
-- Applied at app startup by the migration runner in src-tauri/src/db/mod.rs
--
-- Notes:
--   - PRAGMA journal_mode = WAL and PRAGMA foreign_keys = ON are set at
--     connection open time (Rust DB initialiser, T008). NOT here.
--   - PKs are TEXT (ULID format), except user_preferences (INTEGER singleton)
--     and sync_queue (INTEGER AUTOINCREMENT).
--   - All timestamps are TEXT in ISO 8601 UTC format.
--   - user_preferences default row is seeded at first launch in T012, not here.

-- ---------------------------------------------------------------------------
-- 1. clients
-- ---------------------------------------------------------------------------
CREATE TABLE clients (
    id          TEXT    PRIMARY KEY NOT NULL,
    name        TEXT    NOT NULL UNIQUE,
    color       TEXT    NOT NULL,
    logo_path   TEXT,                          -- nullable; local FS path, never synced
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL,
    modified_at TEXT    NOT NULL
);

-- ---------------------------------------------------------------------------
-- 2. projects  (FK → clients CASCADE)
-- ---------------------------------------------------------------------------
CREATE TABLE projects (
    id          TEXT    PRIMARY KEY NOT NULL,
    client_id   TEXT    NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL,
    modified_at TEXT    NOT NULL,
    UNIQUE (client_id, name)
);

-- ---------------------------------------------------------------------------
-- 3. tasks  (FK → projects CASCADE)
-- ---------------------------------------------------------------------------
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- 4. tags
-- ---------------------------------------------------------------------------
CREATE TABLE tags (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- 5. time_entries
--    project_id and task_id are nullable: SET NULL when the referenced row is
--    deleted (orphan retention — spec US3 acceptance scenario 6).
--    ended_at is nullable: NULL means the timer is currently running.
-- ---------------------------------------------------------------------------
CREATE TABLE time_entries (
    id          TEXT    PRIMARY KEY NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    started_at  TEXT    NOT NULL,
    ended_at    TEXT,                          -- NULL = running timer
    project_id  TEXT    REFERENCES projects(id) ON DELETE SET NULL,
    task_id     TEXT    REFERENCES tasks(id)   ON DELETE SET NULL,
    is_break    INTEGER NOT NULL DEFAULT 0,
    device_id   TEXT    NOT NULL,
    created_at  TEXT    NOT NULL,
    modified_at TEXT    NOT NULL
);

-- At most one running timer per device (partial unique index)
CREATE UNIQUE INDEX idx_time_entries_running
    ON time_entries (device_id)
    WHERE ended_at IS NULL;

-- For paginated list queries sorted by date descending
CREATE INDEX idx_time_entries_started_at
    ON time_entries (started_at);

-- For non-partial queries that filter WHERE ended_at IS NULL
CREATE INDEX idx_time_entries_ended_at
    ON time_entries (ended_at);

-- ---------------------------------------------------------------------------
-- 6. time_entry_tags  (junction: CASCADE on both sides)
--    Deleting a time entry removes its tags; deleting a tag removes it from
--    all entries (junction row only — entry itself is unmodified).
-- ---------------------------------------------------------------------------
CREATE TABLE time_entry_tags (
    time_entry_id TEXT NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    tag_id        TEXT NOT NULL REFERENCES tags(id)         ON DELETE CASCADE,
    PRIMARY KEY (time_entry_id, tag_id)
);

-- ---------------------------------------------------------------------------
-- 7. window_activity_records
--    Standalone table (no FK to time_entries). Insert-only by the tracking
--    loop. synced_at = NULL means pending sync to external DB.
-- ---------------------------------------------------------------------------
CREATE TABLE window_activity_records (
    id             TEXT PRIMARY KEY NOT NULL,
    process_name   TEXT NOT NULL,
    window_title   TEXT NOT NULL,
    window_handle  TEXT NOT NULL,
    recorded_at    TEXT NOT NULL,
    device_id      TEXT NOT NULL,
    synced_at      TEXT             -- nullable; NULL = not yet synced
);

-- For pending-sync batch queries (sync service: WHERE synced_at IS NULL)
CREATE INDEX idx_war_synced
    ON window_activity_records (synced_at)
    WHERE synced_at IS NULL;

-- For timeline/chronological queries
CREATE INDEX idx_war_recorded_at
    ON window_activity_records (recorded_at);

-- ---------------------------------------------------------------------------
-- 8. screenshots
--    Local-only metadata. File content lives on disk. Never synced.
--    trigger is constrained to known values.
-- ---------------------------------------------------------------------------
CREATE TABLE screenshots (
    id           TEXT PRIMARY KEY NOT NULL,
    file_path    TEXT NOT NULL,
    captured_at  TEXT NOT NULL,
    window_title TEXT NOT NULL,
    process_name TEXT NOT NULL,
    trigger      TEXT NOT NULL CHECK (trigger IN ('interval', 'window_change')),
    device_id    TEXT NOT NULL
);

-- For rolling retention cleanup and chronological timeline queries
CREATE INDEX idx_screenshots_captured_at
    ON screenshots (captured_at);

-- ---------------------------------------------------------------------------
-- 9. user_preferences
--    Singleton row (id = 1). CHECK constraint enforces exactly one row per
--    device. Default row is seeded at first launch (T012), not here.
-- ---------------------------------------------------------------------------
CREATE TABLE user_preferences (
    id                                 INTEGER PRIMARY KEY DEFAULT 1,
    local_timezone                     TEXT    NOT NULL DEFAULT 'UTC',
    inactivity_timeout_seconds         INTEGER NOT NULL DEFAULT 300,
    screenshot_interval_seconds        INTEGER NOT NULL DEFAULT 300,
    screenshot_retention_days          INTEGER NOT NULL DEFAULT 30,
    screenshot_storage_path            TEXT,                           -- nullable; defaults to {exe_dir}/screenshots/ at runtime
    timer_notification_threshold_hours REAL    NOT NULL DEFAULT 8.0,
    page_size                          INTEGER NOT NULL DEFAULT 50,
    external_db_uri_stored             INTEGER NOT NULL DEFAULT 0,     -- flag only; URI lives in OS keychain
    external_db_enabled                INTEGER NOT NULL DEFAULT 0,
    notification_channels_json         TEXT,                           -- nullable; JSON array of channel config objects
    process_deny_list_json             TEXT    NOT NULL
        DEFAULT '["keepass","1password","bitwarden","lastpass"]',
    CHECK (id = 1)
);

-- ---------------------------------------------------------------------------
-- 10. sync_queue
--     Tracks pending upserts/deletes to the external database.
--     Rows are deleted from this table once successfully synced.
-- ---------------------------------------------------------------------------
CREATE TABLE sync_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT    NOT NULL,
    record_id   TEXT    NOT NULL,
    operation   TEXT    NOT NULL CHECK (operation IN ('upsert', 'delete')),
    queued_at   TEXT    NOT NULL
);

-- For queue processing in insertion order
CREATE INDEX idx_sync_queue_queued_at
    ON sync_queue (queued_at);
