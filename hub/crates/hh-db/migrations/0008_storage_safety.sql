ALTER TABLE backup_items ADD COLUMN expected_size INTEGER;
-- Durable cleanup pins have no automatic expiry: the mobile system dialog
-- can remain open or the phone can crash. Release only on explicit completion.
CREATE TABLE cleanup_leases (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES backup_sources(id),
  device_id TEXT NOT NULL REFERENCES devices(id),
  created_at INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active','completed'))
);
CREATE TABLE cleanup_lease_items (
  lease_id TEXT NOT NULL REFERENCES cleanup_leases(id),
  file_id TEXT NOT NULL REFERENCES files(id),
  client_item_id TEXT NOT NULL,
  hash TEXT NOT NULL,
  size INTEGER NOT NULL,
  PRIMARY KEY(lease_id,client_item_id)
);
CREATE INDEX cleanup_pins ON cleanup_lease_items(file_id);
CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('queued','running','completed','failed','cancelled','interrupted')),
  total INTEGER NOT NULL DEFAULT 0,
  done INTEGER NOT NULL DEFAULT 0,
  cancel_requested INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL,
  result TEXT,
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
-- No cascading deletion: preserve external copies independently of library.
CREATE TABLE second_copy_files (
  file_id TEXT NOT NULL,
  target_root_id TEXT NOT NULL REFERENCES storage_roots(id),
  rel_path TEXT NOT NULL,
  hash TEXT NOT NULL,
  size INTEGER NOT NULL,
  verified_at INTEGER NOT NULL,
  PRIMARY KEY(file_id,target_root_id)
);
CREATE TABLE file_operation_journal (
  id TEXT PRIMARY KEY,
  file_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  old_path TEXT NOT NULL,
  new_path TEXT NOT NULL,
  payload TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('prepared','done')),
  created_at INTEGER NOT NULL
);
