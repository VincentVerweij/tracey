-- Migration 008: Logging preferences

-- Toggle to enable/disable writing a log file next to the exe
ALTER TABLE user_preferences
    ADD COLUMN logging_enabled INTEGER NOT NULL DEFAULT 0;

-- Minimum severity that gets written to the log file: error | warning | info | trace
ALTER TABLE user_preferences
    ADD COLUMN log_level TEXT NOT NULL DEFAULT 'info';
