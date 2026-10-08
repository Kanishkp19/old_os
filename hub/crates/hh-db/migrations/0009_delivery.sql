CREATE TABLE relay_delivery (
  id TEXT PRIMARY KEY,
  file_id TEXT NOT NULL REFERENCES files(id),
  source_device_id TEXT NOT NULL REFERENCES devices(id),
  target_device_id TEXT NOT NULL REFERENCES devices(id),
  transfer_id TEXT UNIQUE REFERENCES transfers(id),
  hash TEXT NOT NULL,
  size INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','delivered','cancelled')),
  created_at INTEGER NOT NULL,
  delivered_at INTEGER
);
CREATE INDEX relay_target ON relay_delivery(target_device_id,status,created_at);
CREATE TABLE library_moves (
  job_id TEXT PRIMARY KEY REFERENCES jobs(id),
  old_root TEXT NOT NULL,
  new_root TEXT NOT NULL,
  config_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('copying','switching','completed'))
);
ALTER TABLE transfers ADD COLUMN source_stamp TEXT;
