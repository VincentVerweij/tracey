-- Migration 011: Allow the 'suspension_end' screenshot trigger (#79, ADR-0004)
-- SQLite cannot alter a CHECK constraint, so the table is rebuilt.
-- Nothing references screenshots by foreign key.

CREATE TABLE screenshots_new (
    id           TEXT PRIMARY KEY NOT NULL,
    file_path    TEXT NOT NULL,
    captured_at  TEXT NOT NULL,
    window_title TEXT NOT NULL,
    process_name TEXT NOT NULL,
    trigger      TEXT NOT NULL CHECK (trigger IN ('interval', 'window_change', 'suspension_end')),
    device_id    TEXT NOT NULL,
    ocr_text     TEXT
);

INSERT INTO screenshots_new
    (id, file_path, captured_at, window_title, process_name, trigger, device_id, ocr_text)
SELECT id, file_path, captured_at, window_title, process_name, trigger, device_id, ocr_text
FROM screenshots;

DROP TABLE screenshots;
ALTER TABLE screenshots_new RENAME TO screenshots;

CREATE INDEX idx_screenshots_captured_at
    ON screenshots (captured_at);
