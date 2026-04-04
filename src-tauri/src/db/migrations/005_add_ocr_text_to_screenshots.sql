-- Migration 005: Add ocr_text column to screenshots
-- ocr_text is populated asynchronously after capture; initially NULL.
ALTER TABLE screenshots ADD COLUMN ocr_text TEXT;
