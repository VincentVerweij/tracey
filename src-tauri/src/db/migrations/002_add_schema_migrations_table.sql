-- Migration 002: Schema Migrations Tracking Table
-- Applied at app startup by the migration runner in src-tauri/src/db/mod.rs
--
-- This table records which migration files have been applied, so the runner
-- can skip already-applied migrations on subsequent startups.
--
-- The same table structure is used by the external Postgres sync migration
-- runner (T071) for consistency.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version    TEXT PRIMARY KEY NOT NULL,
    applied_at TEXT NOT NULL
);
