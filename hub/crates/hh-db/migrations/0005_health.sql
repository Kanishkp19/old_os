-- 0005: duplicates, disk health, integrity (BACKEND_SCHEMA §5, §6)
CREATE TABLE duplicate_groups (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('exact','likely','similar')),
  hash        TEXT,
  reclaimable_bytes INTEGER NOT NULL,
  detected_at INTEGER NOT NULL,
  status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','resolved','dismissed'))
);
CREATE TABLE duplicate_members (
  group_id TEXT NOT NULL REFERENCES duplicate_groups(id) ON DELETE CASCADE,
  file_id  TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  is_keeper INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (group_id, file_id)
) WITHOUT ROWID;

CREATE TABLE disks (
  id          TEXT PRIMARY KEY,
  model       TEXT,
  serial      TEXT,
  media_type  TEXT CHECK (media_type IN ('hdd','ssd','nvme','usb','unknown')),
  size_bytes  INTEGER,
  first_seen_at INTEGER NOT NULL
);

CREATE TABLE disk_health_snapshots (
  id          TEXT PRIMARY KEY,
  disk_id     TEXT NOT NULL REFERENCES disks(id),
  taken_at    INTEGER NOT NULL,
  health      TEXT NOT NULL CHECK (health IN ('good','caution','failing','unknown')),
  predict_failure INTEGER,
  temperature_c INTEGER,
  power_on_hours INTEGER,
  reallocated_sectors INTEGER,
  pending_sectors INTEGER,
  free_bytes  INTEGER,
  raw_json    TEXT
);
CREATE INDEX idx_health_disk_time ON disk_health_snapshots(disk_id, taken_at DESC);

CREATE TABLE integrity_events (
  id          TEXT PRIMARY KEY,
  file_id     TEXT REFERENCES files(id),
  kind        TEXT NOT NULL CHECK (kind IN ('scrub_ok','hash_mismatch','missing','read_error','repaired_from_copy')),
  detected_at INTEGER NOT NULL,
  detail      TEXT
);

CREATE TABLE second_copy_runs (
  id          TEXT PRIMARY KEY,
  target_root_id TEXT NOT NULL REFERENCES storage_roots(id),
  started_at  INTEGER NOT NULL,
  finished_at INTEGER,
  files_copied INTEGER DEFAULT 0,
  bytes_copied INTEGER DEFAULT 0,
  files_failed INTEGER DEFAULT 0,
  status      TEXT NOT NULL CHECK (status IN ('running','ok','partial','failed'))
);
