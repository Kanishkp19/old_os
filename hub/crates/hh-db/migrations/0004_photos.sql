-- 0004: photo backup and gallery (BACKEND_SCHEMA §4)
CREATE TABLE backup_sources (
  id          TEXT PRIMARY KEY,
  device_id   TEXT NOT NULL REFERENCES devices(id),
  kind        TEXT NOT NULL CHECK (kind IN ('camera_roll','folder')),
  label       TEXT,
  enabled     INTEGER NOT NULL DEFAULT 0,
  approved_at INTEGER,
  wifi_only   INTEGER NOT NULL DEFAULT 1,
  charging_only INTEGER NOT NULL DEFAULT 0,
  last_run_at INTEGER
);

CREATE TABLE backup_items (
  id            TEXT PRIMARY KEY,
  source_id     TEXT NOT NULL REFERENCES backup_sources(id) ON DELETE CASCADE,
  client_item_id TEXT NOT NULL,
  file_id       TEXT REFERENCES files(id),
  hash          TEXT,
  status        TEXT NOT NULL CHECK (status IN ('pending','uploading','verified','failed','skipped')),
  verified_at   INTEGER,
  local_freed_at INTEGER,
  UNIQUE (source_id, client_item_id)
);
CREATE INDEX idx_backup_items_status ON backup_items(status);

CREATE TABLE media (
  file_id     TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
  type        TEXT NOT NULL CHECK (type IN ('photo','video')),
  taken_at    INTEGER NOT NULL,
  taken_at_source TEXT NOT NULL CHECK (taken_at_source IN ('exif','container','mtime','upload')),
  width       INTEGER,
  height      INTEGER,
  duration_ms INTEGER,
  orientation INTEGER,
  camera_make TEXT,
  camera_model TEXT,
  gps_lat     REAL,
  gps_lon     REAL,
  phash       INTEGER,
  thumb_status TEXT NOT NULL DEFAULT 'pending' CHECK (thumb_status IN ('pending','ready','failed','unsupported')),
  thumb_256_path TEXT,
  thumb_1024_path TEXT
);
CREATE INDEX idx_media_taken ON media(taken_at DESC, file_id);
CREATE INDEX idx_media_phash ON media(phash) WHERE phash IS NOT NULL;

CREATE TABLE albums (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE album_items (
  album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
  file_id  TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  added_at INTEGER NOT NULL,
  PRIMARY KEY (album_id, file_id)
) WITHOUT ROWID;
