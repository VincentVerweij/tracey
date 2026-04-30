-- Migration 009: Minimize to system tray preference

ALTER TABLE user_preferences
    ADD COLUMN minimize_to_tray INTEGER NOT NULL DEFAULT 0;
