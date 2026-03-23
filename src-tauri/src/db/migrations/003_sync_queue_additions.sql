-- Migration 003: Add attempts column to sync_queue
--
-- The SyncService (T072/T073) needs per-row retry counting to implement
-- exponential backoff and to prevent a single poisoned queue entry from
-- blocking all subsequent syncs.
--
-- `attempts` is NOT payload storage — the sync service re-reads the latest
-- record from local SQLite at sync time (correct for last-write-wins semantics).

ALTER TABLE sync_queue ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
