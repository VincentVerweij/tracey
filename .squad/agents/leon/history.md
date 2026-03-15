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
