-- Rotate bounded scrub batches even when a damaged file cannot be repaired.
CREATE INDEX idx_integrity_events_file_time ON integrity_events(file_id,detected_at DESC);
