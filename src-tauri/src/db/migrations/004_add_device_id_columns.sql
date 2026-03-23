-- Migration 004: Add device_id to entity tables
-- 
-- clients, projects, tasks, and tags were created without a device_id column.
-- The external Postgres schema requires it for multi-device attribution.
--
-- Existing rows receive '' as the default; the sync service substitutes the
-- local COMPUTERNAME/HOSTNAME value at sync time (see sync_service::device_id()).

ALTER TABLE clients  ADD COLUMN device_id TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN device_id TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks    ADD COLUMN device_id TEXT NOT NULL DEFAULT '';
ALTER TABLE tags     ADD COLUMN device_id TEXT NOT NULL DEFAULT '';
