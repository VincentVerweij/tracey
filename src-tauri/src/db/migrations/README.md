# Database Migrations

Sequential SQL migrations applied at app startup by the migration runner in `src-tauri/src/db/mod.rs`.

## Rules
- Files are named `{NNN}_{description}.sql` (zero-padded, sequential, no gaps)
- Migrations are applied in lexicographic order
- Applied migrations are recorded in `schema_migrations` table
- Never edit an applied migration — add a new one instead
- All timestamps are ISO 8601 UTC (TEXT columns)
- PKs are ULIDs stored as TEXT

## Tables
- `clients` — client organizations
- `projects` — projects scoped to a client
- `tasks` — tasks scoped to a project
- `tags` — reusable labels for time entries
- `time_entries` — individual work sessions (ended_at = NULL means running)
- `time_entry_tags` — many-to-many junction
- `window_activity_records` — passive window tracking data
- `screenshots` — screenshot file metadata
- `user_preferences` — single-row device configuration
- `sync_queue` — pending writes to external database
- `schema_migrations` — applied migration tracking
