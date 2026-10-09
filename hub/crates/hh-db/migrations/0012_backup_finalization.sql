-- Keep the completed upload that established a backup item. Existing rows
-- remain valid; deduplicated items may refer directly to a verified file.
ALTER TABLE backup_items ADD COLUMN transfer_id TEXT REFERENCES transfers(id);
