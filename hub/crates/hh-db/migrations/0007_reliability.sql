-- Durable transfer intent is committed before the filesystem rename.
ALTER TABLE transfers ADD COLUMN backup_source_id TEXT REFERENCES backup_sources(id);
CREATE TABLE transfer_finalizations (
 transfer_id TEXT PRIMARY KEY REFERENCES transfers(id),
 file_id TEXT NOT NULL UNIQUE,
 root_id TEXT NOT NULL REFERENCES storage_roots(id),
 rel_path TEXT NOT NULL,
 name TEXT NOT NULL,
 category TEXT NOT NULL,
 hash TEXT NOT NULL,
 created_at INTEGER NOT NULL
);
