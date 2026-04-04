-- Migration 007: Auto-classification loop and active learning support

-- classified_at on window_activity_records: NULL = not yet classified
ALTER TABLE window_activity_records ADD COLUMN classified_at TEXT;

CREATE INDEX idx_war_unclassified
    ON window_activity_records (classified_at)
    WHERE classified_at IS NULL;

-- source on time_entries: manual (default), auto (created by classification loop), continued
ALTER TABLE time_entries ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'
    CHECK (source IN ('manual', 'auto', 'continued'));

-- classification_events: full audit trail for the Classification page (Plan D)
CREATE TABLE classification_events (
    id                     TEXT PRIMARY KEY NOT NULL,
    war_id                 TEXT NOT NULL,   -- window_activity_records.id
    process_name           TEXT NOT NULL,
    window_title           TEXT NOT NULL,
    client_id              TEXT,
    project_id             TEXT,
    task_id                TEXT,
    confidence             REAL NOT NULL DEFAULT 0.0,
    classification_source  TEXT NOT NULL DEFAULT 'unclassified',
    -- 'heuristic' | 'tf_idf' | 'unclassified'
    outcome                TEXT NOT NULL DEFAULT 'pending',
    -- 'auto' | 'user_confirmed' | 'user_corrected' | 'unclassified' | 'pending'
    ocr_text               TEXT,            -- raw OCR snippet used during classification
    created_at             TEXT NOT NULL
);

CREATE INDEX idx_classification_events_created_at
    ON classification_events (created_at DESC);

-- Auto-classification preferences (added to user_preferences singleton)
ALTER TABLE user_preferences
    ADD COLUMN auto_classification_enabled INTEGER NOT NULL DEFAULT 1;

ALTER TABLE user_preferences
    ADD COLUMN auto_classification_confidence_threshold REAL NOT NULL DEFAULT 0.7;

ALTER TABLE user_preferences
    ADD COLUMN auto_classification_group_gap_seconds INTEGER NOT NULL DEFAULT 120;

-- JSON array of { pattern_key, dismissed_count, last_snoozed_at }
ALTER TABLE user_preferences
    ADD COLUMN classification_snooze_json TEXT;
