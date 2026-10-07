-- 0003: transfers (BACKEND_SCHEMA §3)
CREATE TABLE transfers (
  id            TEXT PRIMARY KEY,
  device_id     TEXT NOT NULL REFERENCES devices(id),
  direction     TEXT NOT NULL CHECK (direction IN ('upload','download','relay')),
  kind          TEXT NOT NULL DEFAULT 'send' CHECK (kind IN ('send','backup','import','second_copy')),
  name          TEXT NOT NULL,
  size          INTEGER NOT NULL,
  mime          TEXT,
  rel_path      TEXT,
  chunk_size    INTEGER NOT NULL,
  chunk_count   INTEGER NOT NULL,
  expected_root_hash TEXT,
  client_item_id TEXT,
  tmp_path      TEXT,
  status        TEXT NOT NULL CHECK (status IN ('open','verifying','completed','failed','aborted','expired')),
  error_code    TEXT,
  bytes_verified INTEGER NOT NULL DEFAULT 0,
  result_file_id TEXT REFERENCES files(id),
  target_device_id TEXT REFERENCES devices(id),
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  completed_at  INTEGER,
  expires_at    INTEGER NOT NULL
);
CREATE INDEX idx_transfers_device_status ON transfers(device_id, status);
CREATE INDEX idx_transfers_updated ON transfers(updated_at DESC);

CREATE TABLE transfer_chunks (
  transfer_id TEXT NOT NULL REFERENCES transfers(id) ON DELETE CASCADE,
  idx         INTEGER NOT NULL,
  hash        TEXT NOT NULL,
  size        INTEGER NOT NULL,
  verified_at INTEGER NOT NULL,
  PRIMARY KEY (transfer_id, idx)
) WITHOUT ROWID;
