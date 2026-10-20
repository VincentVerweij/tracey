-- Migration 012: Presence spans for the Timeline bar (#106, #123, ADR-0006)
-- Written only by the presence recorder. Only tracked states are stored;
-- every gap between spans is Untracked. Local only: never synced, and pruned
-- with screenshots by screenshot_retention_days (ADR-0006 decision 7).

CREATE TABLE presence_spans (
    id         TEXT PRIMARY KEY NOT NULL,
    started_at TEXT NOT NULL,
    ended_at   TEXT NOT NULL,
    state      TEXT NOT NULL CHECK (state IN ('Present', 'Away')),
    away_cause TEXT CHECK (away_cause IN ('Locked', 'Idle')),
    CHECK ((state = 'Away') = (away_cause IS NOT NULL)),
    CHECK (ended_at >= started_at)
);

CREATE INDEX idx_presence_spans_started_at
    ON presence_spans (started_at);
