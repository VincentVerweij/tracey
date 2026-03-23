# Leon — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** SQLite (`rusqlite` on Rust side, `Microsoft.Data.Sqlite` on C# side), WAL mode, Postgres/Supabase sync
- **My files:** `specs/001-window-activity-tracker/data-model.md` (source of truth), `src-tauri/src/db/migrations/`
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup & Schema Baseline
- Entity hierarchy: Client 1──* Project 1──* Task / TimeEntry *──* Tag / WindowActivityRecord / Screenshot
- PKs: ULIDs (TEXT) for lexicographic ordering. All timestamps ISO 8601 UTC.
- WAL mode: `PRAGMA journal_mode = WAL` + `PRAGMA foreign_keys = ON` at DB open (Reese applies)
- Tables: clients, projects, tasks, tags, time_entries, time_entry_tags, window_activity_records, screenshots, user_preferences, sync_queue
- What is NEVER synced: `screenshots` table, `user_preferences` table, `sync_queue` table, `logo_path` field on clients
- Conflict resolution: last-write-wins on `modified_at` field (sync strategy)
- Orphan rule: deleting a client cascade-deletes projects/tasks but NOT time entries — they become orphaned (retain historical data)
- Process deny-list: stored as JSON in `user_preferences.process_deny_list_json`, applied at Rust collection boundary
- Migration runner is sequential — numbered files applied at startup, no rollbacks, no branching
- Max scale: ~1M window-activity events/year. Screenshot retention: 30 days rolling.

### 2026-03-15: T009 — DDL Migration Files Written
- Created `src-tauri/src/db/migrations/001_initial_schema.sql` — all 10 tables in dependency order
- Created `src-tauri/src/db/migrations/002_add_schema_migrations_table.sql` — migration tracking table
- Created `src-tauri/src/db/migrations/README.md` — conventions guide

**FK behavior decisions:**
- `projects.client_id` → `clients(id)` ON DELETE CASCADE
- `tasks.project_id` → `projects(id)` ON DELETE CASCADE
- `time_entries.project_id` → `projects(id)` ON DELETE SET NULL (orphan retention, US3 scenario 6)
- `time_entries.task_id` → `tasks(id)` ON DELETE SET NULL (same reason)
- `time_entry_tags.time_entry_id` → `time_entries(id)` ON DELETE CASCADE
- `time_entry_tags.tag_id` → `tags(id)` ON DELETE CASCADE
- `window_activity_records` — no FK to time_entries; standalone insert-only table
- `screenshots` — standalone; no FK anywhere

**Index decisions (beyond data-model.md baseline):**
- `idx_time_entries_started_at` — paginated list sorted by date
- `idx_time_entries_ended_at` — non-partial filter for WHERE ended_at IS NULL queries
- `idx_time_entries_running` (partial UNIQUE) — enforce one running timer per device
- `idx_war_synced` (partial) — pending sync batch (WHERE synced_at IS NULL)
- `idx_war_recorded_at` — chronological timeline queries
- `idx_screenshots_captured_at` — rolling retention cleanup + timeline
- `idx_sync_queue_queued_at` — queue processing order

**Deviations/decisions flagged for Reese (migration runner, T008/T009):**
- PRAGMA journal_mode = WAL + PRAGMA foreign_keys = ON are NOT in migrations; applied at connection open time
- user_preferences seed INSERT omitted from 001 — T012 handles first-launch seeding
- sync_queue structure follows data-model.md (`table_name`, `record_id`, `queued_at`) NOT the task brief (`entity_type`, `enqueued_at`, `payload`, `attempts`) — data-model.md is authoritative
- user_preferences.id is INTEGER (singleton pattern, CHECK id = 1), not TEXT ULID
