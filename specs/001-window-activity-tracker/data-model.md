# Data Model: Window Activity Timetracking Tool

**Phase**: 1 — Design & Contracts  
**Branch**: `001-window-activity-tracker`  
**Date**: 2026-03-14

---

## Entity Overview

```
Client 1──* Project 1──* Task
                │
           TimeEntry *──* Tag
                │
         WindowActivityRecord
                │
           Screenshot
```

---

## Entities

### Client

Represents a client organization that owns one or more projects.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | ULID preferred for lexicographic ordering |
| `name` | TEXT | NOT NULL, UNIQUE | Display name |
| `color` | TEXT | NOT NULL | Hex color string e.g. `#3B82F6` |
| `logo_path` | TEXT | NULLABLE | Absolute path to logo image on local FS |
| `is_archived` | BOOLEAN | NOT NULL, DEFAULT false | Soft-delete; hidden from pickers |
| `created_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `modified_at` | TEXT | NOT NULL | ISO 8601 UTC; used for last-write-wins sync |

**Validation rules**:
- `name` must be non-empty; max 100 characters.
- `color` must be a valid 6-char hex color (`#RRGGBB`).
- `logo_path`, if set, must point to a readable file on the local filesystem.

**State transitions**:
- `active` (is_archived = false) → `archived` (is_archived = true) — reversible at any time.
- `archived` → `active` — unarchive.
- Deletion cascades to all child projects and tasks.

---

### Project

Represents a work project scoped to exactly one client.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `client_id` | UUID (TEXT) | FK → Client.id, NOT NULL | |
| `name` | TEXT | NOT NULL | Unique within client; may repeat across clients |
| `is_archived` | BOOLEAN | NOT NULL, DEFAULT false | |
| `created_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `modified_at` | TEXT | NOT NULL | ISO 8601 UTC |

**Validation rules**:
- `name` must be non-empty; max 100 characters.
- `name` must be unique within its `client_id`.

**State transitions**:
- Same as Client: active ↔ archived; deletion cascades to tasks.

---

### Task

Represents a specific task under a project.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `project_id` | UUID (TEXT) | FK → Project.id, NOT NULL | Cascade delete with project |
| `name` | TEXT | NOT NULL | |
| `created_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `modified_at` | TEXT | NOT NULL | ISO 8601 UTC |

**Validation rules**:
- `name` must be non-empty; max 100 characters.

---

### Tag

A predefined label created by the user; not creatable on-the-fly during entry logging.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `name` | TEXT | NOT NULL, UNIQUE | |
| `created_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `modified_at` | TEXT | NOT NULL | ISO 8601 UTC |

**Validation rules**:
- `name` must be non-empty; max 50 characters.
- Deletion removes all entries in `TimeEntryTag` junction table; time entries are otherwise unmodified.

---

### TimeEntry

The central entity. Represents a logged period of work (or a running timer when `ended_at` is NULL).

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `description` | TEXT | NOT NULL | May be empty string |
| `started_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `ended_at` | TEXT | NULLABLE | NULL = timer currently running |
| `project_id` | UUID (TEXT) | NULLABLE, FK → Project.id | Set to NULL if project deleted |
| `task_id` | UUID (TEXT) | NULLABLE, FK → Task.id | Set to NULL if task deleted |
| `is_break` | BOOLEAN | NOT NULL, DEFAULT false | True for break entries created by idle-return prompt |
| `device_id` | TEXT | NOT NULL | UUID identifying the device that created the entry |
| `created_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `modified_at` | TEXT | NOT NULL | ISO 8601 UTC; used for last-write-wins conflict resolution |

**Validation rules**:
- `ended_at` must be NULL or a datetime after `started_at`.
- Only one `TimeEntry` with `ended_at IS NULL` may exist at any time (enforced at application level and via a partial unique index).
- Exactly zero or one running timer at any moment.

**State transitions**:
```
[running: ended_at = NULL]
       │
  stop()/new timer starts
       │
[completed: ended_at = <datetime>]
       │
  continue() → new TimeEntry
```

---

### TimeEntryTag (junction)

Many-to-many link between TimeEntry and Tag.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `time_entry_id` | UUID (TEXT) | FK → TimeEntry.id, NOT NULL | Cascade delete with entry |
| `tag_id` | UUID (TEXT) | FK → Tag.id, NOT NULL | Set-delete on tag deletion |
| PK | — | (time_entry_id, tag_id) | |

---

### WindowActivityRecord

A timestamped snapshot of the user's active process and window.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `process_name` | TEXT | NOT NULL | e.g. `code.exe`; max 260 chars (MAX_PATH) |
| `window_title` | TEXT | NOT NULL | Truncated to 500 chars; may contain PII — redacted in logs |
| `window_handle` | TEXT | NOT NULL | Hex string representation of HWND |
| `recorded_at` | TEXT | NOT NULL | ISO 8601 UTC; 1-second resolution |
| `device_id` | TEXT | NOT NULL | |
| `synced_at` | TEXT | NULLABLE | ISO 8601 UTC; NULL = pending sync to external DB |

**Notes**:
- Written to local SQLite first; batched and flushed to external DB every **30 seconds** (SC-007).
- `window_title` values are screened against a configurable deny-list of process names (e.g., `KeePass`, `1Password`) and replaced with `[REDACTED]` before storage (Constitution V – privacy).
- Records are never returned to the UI; they are internal tracing data for future ML use.

---

### Screenshot

Metadata record for a locally captured screenshot image. Stored only in local SQLite; **never synced** to the external database.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | UUID (TEXT) | PK, NOT NULL | |
| `file_path` | TEXT | NOT NULL | Absolute path to the JPEG file on the local filesystem |
| `captured_at` | TEXT | NOT NULL | ISO 8601 UTC |
| `window_title` | TEXT | NOT NULL | Title at time of capture; truncated to 500 chars |
| `process_name` | TEXT | NOT NULL | |
| `trigger` | TEXT | NOT NULL | `interval` or `window_change` |
| `device_id` | TEXT | NOT NULL | |

**Notes**:
- Screenshot files are JPEG at 50 % scale of the active monitor resolution.
- This table is local-only — never synced to external DB (FR-062).
- Records are deleted alongside their corresponding file during rolling retention cleanup.

---

### UserPreferences

Single-row per device; singleton pattern.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | INTEGER | PK = 1 (singleton) | |
| `local_timezone` | TEXT | NOT NULL, DEFAULT 'UTC' | IANA timezone name e.g. `Europe/Amsterdam` |
| `inactivity_timeout_seconds` | INTEGER | NOT NULL, DEFAULT 300 | Min: 60 |
| `screenshot_interval_seconds` | INTEGER | NOT NULL, DEFAULT 60 | Min: 10 |
| `screenshot_retention_days` | INTEGER | NOT NULL, DEFAULT 30 | Min: 1 |
| `screenshot_storage_path` | TEXT | NULLABLE | Defaults to `{exe_dir}/screenshots` if NULL |
| `timer_notification_threshold_hours` | REAL | NOT NULL, DEFAULT 8.0 | |
| `page_size` | INTEGER | NOT NULL, DEFAULT 50 | Time entry list page size |
| `external_db_uri_stored` | BOOLEAN | NOT NULL, DEFAULT false | True when a URI is saved in OS keychain |
| `external_db_enabled` | BOOLEAN | NOT NULL, DEFAULT false | |
| `notification_channels_json` | TEXT | NULLABLE | JSON array of channel config objects |
| `process_deny_list_json` | TEXT | NOT NULL, DEFAULT '["keepass","1password","bitwarden"]' | JSON array of process name substrings to redact |

**Notes**:
- `external_db_uri_stored = true` means the URI lives in the OS credential store (Windows Credential Manager via `keyring` crate). The actual URI is never written to SQLite.
- `notification_channels_json` stores an array of `{ channelId: string, settings: object, enabled: boolean }` values. Kept as JSON to support extensible channel types without schema changes.

---

## SQLite Schema (DDL)

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE clients (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL UNIQUE,
    color        TEXT NOT NULL,
    logo_path    TEXT,
    is_archived  INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    modified_at  TEXT NOT NULL
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY NOT NULL,
    client_id   TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    UNIQUE (client_id, name)
);

CREATE TABLE tasks (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE TABLE tags (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE TABLE time_entries (
    id          TEXT PRIMARY KEY NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    project_id  TEXT REFERENCES projects(id) ON DELETE SET NULL,
    task_id     TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    is_break    INTEGER NOT NULL DEFAULT 0,
    device_id   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

-- Enforce at most one running timer
CREATE UNIQUE INDEX idx_time_entries_running
    ON time_entries (device_id) WHERE ended_at IS NULL;

CREATE TABLE time_entry_tags (
    time_entry_id TEXT NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    tag_id        TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (time_entry_id, tag_id)
);

CREATE TABLE window_activity_records (
    id             TEXT PRIMARY KEY NOT NULL,
    process_name   TEXT NOT NULL,
    window_title   TEXT NOT NULL,
    window_handle  TEXT NOT NULL,
    recorded_at    TEXT NOT NULL,
    device_id      TEXT NOT NULL,
    synced_at      TEXT
);

CREATE INDEX idx_war_synced ON window_activity_records (synced_at) WHERE synced_at IS NULL;

CREATE TABLE screenshots (
    id           TEXT PRIMARY KEY NOT NULL,
    file_path    TEXT NOT NULL,
    captured_at  TEXT NOT NULL,
    window_title TEXT NOT NULL,
    process_name TEXT NOT NULL,
    trigger      TEXT NOT NULL CHECK (trigger IN ('interval','window_change')),
    device_id    TEXT NOT NULL
);

CREATE INDEX idx_screenshots_captured ON screenshots (captured_at);

CREATE TABLE user_preferences (
    id                                  INTEGER PRIMARY KEY DEFAULT 1,
    local_timezone                      TEXT NOT NULL DEFAULT 'UTC',
    inactivity_timeout_seconds          INTEGER NOT NULL DEFAULT 300,
    screenshot_interval_seconds         INTEGER NOT NULL DEFAULT 60,
    screenshot_retention_days           INTEGER NOT NULL DEFAULT 30,
    screenshot_storage_path             TEXT,
    timer_notification_threshold_hours  REAL NOT NULL DEFAULT 8.0,
    page_size                           INTEGER NOT NULL DEFAULT 50,
    external_db_uri_stored              INTEGER NOT NULL DEFAULT 0,
    external_db_enabled                 INTEGER NOT NULL DEFAULT 0,
    notification_channels_json          TEXT,
    process_deny_list_json              TEXT NOT NULL
        DEFAULT '["keepass","1password","bitwarden","lastpass"]',
    CHECK (id = 1)
);

INSERT INTO user_preferences DEFAULT VALUES;
```

---

## External Database Schema (Postgres / Supabase)

The external DB uses the same logical schema but in Postgres syntax. Differences:
- `id` is `UUID` type.
- `BOOLEAN` instead of `INTEGER 0/1`.
- `TIMESTAMPTZ` instead of `TEXT` for datetimes.
- `device_id` is present on all synced tables to identify origin device.

Tables synced: `clients`, `projects`, `tasks`, `tags`, `time_entries`, `time_entry_tags`, `window_activity_records`, `user_preferences`.  
Tables NOT synced: screenshots are never stored in the database; they live solely on the local filesystem.

### `user_preferences` in Postgres

Because preferences are per-device, the external DB uses `device_id` as the primary key instead of the local singleton integer `id = 1`. Each device upserts its own row:

```sql
CREATE TABLE user_preferences (
    device_id                           UUID PRIMARY KEY,
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
```

**Notes**:
- `external_db_uri_stored` is not synced (it is a local-device flag for the OS keychain).
- `modified_at` enables last-write-wins resolution if the same device's preferences are changed on different devices sharing a URI.

---

## Sync State Tracking

A local `sync_queue` table tracks pending upserts to the external DB:

```sql
CREATE TABLE sync_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT    NOT NULL,
    record_id   TEXT    NOT NULL,
    operation   TEXT    NOT NULL CHECK (operation IN ('upsert','delete')),
    queued_at   TEXT    NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0  -- incremented on each failed sync attempt
);
```

The sync service processes this queue in order and marks records as synced by deleting them from the queue.
