-- Stable caller review IDs recover durable cleanup pins after lost replies.
ALTER TABLE cleanup_leases ADD COLUMN client_review_id TEXT;
CREATE UNIQUE INDEX cleanup_review_id ON cleanup_leases(device_id,source_id,client_review_id) WHERE client_review_id IS NOT NULL;
