-- Migration 009: Theme preference (light / dark / system)

ALTER TABLE user_preferences
    ADD COLUMN theme TEXT NOT NULL DEFAULT 'system';
