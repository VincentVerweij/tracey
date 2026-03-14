# External Database Sync API Contract

**Phase**: 1 — Design & Contracts  
**Branch**: `001-window-activity-tracker`  
**Date**: 2026-03-14

This document defines the schema that Tracey writes to the user-supplied external PostgreSQL / Supabase database. Any device sharing the same connection URI reads and writes to this schema.

---

## Design Principles

- All sync is **upsert-based**: each record has a `device_id` and `modified_at` to enable last-write-wins conflict resolution.
- Tracey **never deletes** records from the external DB except when the user explicitly deletes a client (cascades to its children). Orphaned references are preserved as NULL FKs.
- Screenshots are **never** written to the external DB; they live solely on the local filesystem.
- All datetimes are `TIMESTAMPTZ` (UTC).

---

## Schema Version

`tracey_schema_version = 1`

A `schema_migrations` table is used to track applied migrations:

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## Tables

### `clients`

```sql
CREATE TABLE clients (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    color       TEXT NOT NULL,
    logo_path   TEXT,           -- null on external DB (local path only)
    is_archived BOOLEAN NOT NULL DEFAULT false,
    device_id   UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);
```

---

### `projects`

```sql
CREATE TABLE projects (
    id          UUID PRIMARY KEY,
    client_id   UUID NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    device_id   UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL,
    UNIQUE (client_id, name)
);
```

---

### `tasks`

```sql
CREATE TABLE tasks (
    id          UUID PRIMARY KEY,
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    device_id   UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);
```

---

### `tags`

```sql
CREATE TABLE tags (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    device_id   UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);
```

---

### `time_entries`

```sql
CREATE TABLE time_entries (
    id          UUID PRIMARY KEY,
    description TEXT NOT NULL DEFAULT '',
    started_at  TIMESTAMPTZ NOT NULL,
    ended_at    TIMESTAMPTZ,           -- null = timer running
    project_id  UUID REFERENCES projects(id) ON DELETE SET NULL,
    task_id     UUID REFERENCES tasks(id) ON DELETE SET NULL,
    is_break    BOOLEAN NOT NULL DEFAULT false,
    device_id   UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL
);

-- At most one running timer per device
CREATE UNIQUE INDEX idx_time_entries_running
    ON time_entries (device_id) WHERE ended_at IS NULL;
```

---

### `time_entry_tags`

```sql
CREATE TABLE time_entry_tags (
    time_entry_id UUID NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    tag_id        UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (time_entry_id, tag_id)
);
```

---

### `window_activity_records`

```sql
CREATE TABLE window_activity_records (
    id             UUID PRIMARY KEY,
    process_name   TEXT NOT NULL,
    window_title   TEXT NOT NULL,
    window_handle  TEXT NOT NULL,
    recorded_at    TIMESTAMPTZ NOT NULL,
    device_id      UUID NOT NULL,
    synced_at      TIMESTAMPTZ
);

CREATE INDEX idx_war_device_recorded ON window_activity_records (device_id, recorded_at DESC);
```

---

### `user_preferences`

Synced per device using `device_id` as the primary key. Each device owns exactly one row.

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
- Upsert key is `device_id`; last-write-wins on `modified_at`.

---

## Sync Protocol

### Upsert Pattern (last-write-wins)

For each record pending sync:

```sql
INSERT INTO <table> (<columns>)
VALUES (<values>)
ON CONFLICT (id) DO UPDATE SET
    <all_columns_except_id> = EXCLUDED.<column>,
    modified_at = GREATEST(<table>.modified_at, EXCLUDED.modified_at)
WHERE EXCLUDED.modified_at >= <table>.modified_at;
```

`GREATEST` ensures that whichever device has the more recent `modified_at` wins.

### Delete Pattern

When the user explicitly deletes a client:
```sql
DELETE FROM clients WHERE id = $1;
-- Cascades to projects, tasks, time_entries via FK ON DELETE CASCADE
```

### Sync Batch Size

- Time entries: synced in batches of up to 50 per sync cycle.
- Window activity records: synced in batches of up to 500 per sync cycle (higher volume).
- Sync cycles run every 30 seconds when online; triggered immediately after any local write.

### Cross-Device Timer Detection

To check if another device has a running timer visible to the current device:

```sql
SELECT te.*, p.name AS project_name, t.name AS task_name
FROM time_entries te
LEFT JOIN projects p ON te.project_id = p.id
LEFT JOIN tasks    t ON te.task_id    = t.id
WHERE te.ended_at IS NULL
ORDER BY te.started_at DESC;
```

The result includes timers from all devices sharing the connection URI. The UI shows them all; only the local device's timer can be stopped from this device (by matching `device_id`).

---

## Row-Level Security (Supabase / Postgres)

When using Supabase, the user is advised (but not required) to apply RLS policies restricting access by connection string. Since Tracey has no built-in auth system, the external DB is protected solely by the connection URI password. **The user is responsible for securing the database.**

The following advisory RLS config is documented in quick-start notes, not enforced by the app:

```sql
-- Example: allow only the service role (i.e., connection URI owner) full access
ALTER TABLE time_entries ENABLE ROW LEVEL SECURITY;
CREATE POLICY "owner_all" ON time_entries USING (true);  -- unrestricted for single-owner URI
```
