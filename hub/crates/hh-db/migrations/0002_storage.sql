-- 0002: storage and files (BACKEND_SCHEMA §2)
CREATE TABLE storage_roots (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('library','second_copy','import_source')),
  path        TEXT NOT NULL,
  label       TEXT,
  disk_id     TEXT REFERENCES disks(id),
  is_active   INTEGER NOT NULL DEFAULT 1,
  created_at  INTEGER NOT NULL
);

CREATE TABLE files (
  id            TEXT PRIMARY KEY,
  root_id       TEXT NOT NULL REFERENCES storage_roots(id),
  rel_path      TEXT NOT NULL,
  name          TEXT NOT NULL,
  category      TEXT NOT NULL CHECK (category IN ('photo','video','document','music','download','backup','other')),
  mime          TEXT,
  size          INTEGER NOT NULL,
  hash          TEXT NOT NULL,
  chunk_size    INTEGER NOT NULL DEFAULT 4194304,
  source_mode   TEXT NOT NULL DEFAULT 'upload' CHECK (source_mode IN ('upload','import','keep_in_place')),
  origin_device_id TEXT REFERENCES devices(id),
  client_item_id TEXT,
  created_at    INTEGER NOT NULL,
  modified_at   INTEGER,
  last_verified_at INTEGER,
  deleted_at    INTEGER,
  UNIQUE (root_id, rel_path)
);
CREATE INDEX idx_files_hash ON files(hash);
CREATE INDEX idx_files_category_created ON files(category, created_at DESC);
CREATE INDEX idx_files_device_item ON files(origin_device_id, client_item_id);
CREATE INDEX idx_files_deleted ON files(deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE file_chunks (
  file_id   TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  idx       INTEGER NOT NULL,
  hash      TEXT NOT NULL,
  PRIMARY KEY (file_id, idx)
) WITHOUT ROWID;

CREATE TABLE trash (
  file_id     TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
  trashed_at  INTEGER NOT NULL,
  trashed_by_device_id TEXT REFERENCES devices(id),
  purge_after INTEGER NOT NULL
);
