-- Migration 006: Classification engine tables and columns

-- labeled_samples: user-confirmed or user-corrected training data for TF-IDF
CREATE TABLE labeled_samples (
    id            TEXT PRIMARY KEY NOT NULL,
    feature_text  TEXT NOT NULL,   -- normalized "process_name window_title ocr_text"
    process_name  TEXT NOT NULL,
    modified_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    window_title  TEXT NOT NULL,
    client_id     TEXT,
    project_id    TEXT,
    task_id       TEXT,
    source        TEXT NOT NULL CHECK (source IN ('user_confirmed', 'user_corrected', 'auto')),
    device_id     TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    synced_at     TEXT            -- NULL = pending sync to external DB
);

CREATE INDEX idx_labeled_samples_created_at
    ON labeled_samples (created_at);

CREATE INDEX idx_labeled_samples_synced
    ON labeled_samples (synced_at)
    WHERE synced_at IS NULL;

-- classifier_model: serialized TF-IDF model weights (JSON blob)
CREATE TABLE classifier_model (
    id           TEXT PRIMARY KEY NOT NULL,
    model_json   TEXT NOT NULL,
    trained_at   TEXT NOT NULL,
    sample_count INTEGER NOT NULL,
    device_id    TEXT NOT NULL
);

-- classification_rules_json: user-editable heuristic rules (JSON array of HeuristicRule)
ALTER TABLE user_preferences
    ADD COLUMN classification_rules_json TEXT;
-- NULL = no rules configured; the engine treats NULL the same as an empty array.
