# Home Hub — Backend Schema

**Engine:** SQLite 3.40+ (WAL mode) · **Location:** `%ProgramData%\HomeHub\hub.db`
**Conventions:** IDs are ULID (TEXT, sortable). Timestamps are UTC unix epoch milliseconds (INTEGER). Hashes are lowercase BLAKE3 hex. Booleans are INTEGER 0/1. All FKs enforced (`PRAGMA foreign_keys=ON`).

Windows product additions use forward-only migrations `0007`–`0011`: transfer finalization journals and backup-source links; durable cleanup leases, jobs and per-file second-copy records; staged relay deliveries and library-move switching; retriable certificate renewal; and unique device/source cleanup review IDs. `cleanup_leases.client_review_id` is nullable for older clients. Private Notes, browser data and playlists live in the signed-in Windows user's separate application database, never in this Hub database. Before migrating an existing database, the service makes a consistent SQLite `VACUUM INTO` snapshot and retains three recent migration backups.

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;          -- data integrity over speed
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

---

## 1. Core / identity

```sql
CREATE TABLE hub (
  id            TEXT PRIMARY KEY,            -- hub_id (ULID), single row
  name          TEXT NOT NULL,               -- "Kanishk's Home Hub"
  created_at    INTEGER NOT NULL,
  ca_cert_pem   TEXT NOT NULL,
  ca_key_ref    TEXT NOT NULL,               -- DPAPI-protected key blob path/ref, never raw key in DB
  server_cert_pem TEXT NOT NULL,
  schema_version INTEGER NOT NULL
);

CREATE TABLE settings (
  key     TEXT PRIMARY KEY,
  value   TEXT NOT NULL,                     -- JSON
  updated_at INTEGER NOT NULL
);
-- keys: library_root, network.hotspot_enabled, update.auto_check, thumbs.enabled,
--       scrub.rate_pct, trash.retention_days, remote.enabled, telemetry.opt_in

CREATE TABLE devices (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,               -- "Kanishk's Pixel"
  platform      TEXT NOT NULL CHECK (platform IN ('android','ios','macos','windows','linux','web')),
  model         TEXT,
  app_version   TEXT,
  public_key    TEXT NOT NULL,
  cert_pem      TEXT NOT NULL,
  cert_serial   TEXT NOT NULL UNIQUE,
  cert_expires_at INTEGER NOT NULL,
  scopes        TEXT NOT NULL DEFAULT 'files,transfer,photos',  -- csv: files,transfer,photos,remote,admin
  mac_address   TEXT,                        -- for WoL (client's own not needed; hub MAC stored on hub_network)
  paired_at     INTEGER NOT NULL,
  last_seen_at  INTEGER,
  last_ip       TEXT,
  status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','revoked')),
  revoked_at    INTEGER
);
CREATE INDEX idx_devices_status ON devices(status);

CREATE TABLE revoked_certs (
  cert_serial TEXT PRIMARY KEY,
  device_id   TEXT NOT NULL REFERENCES devices(id),
  revoked_at  INTEGER NOT NULL,
  reason      TEXT
);

CREATE TABLE pairing_tokens (
  id          TEXT PRIMARY KEY,
  token_hash  TEXT NOT NULL UNIQUE,          -- sha256(token); raw token never stored
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL,
  used_at     INTEGER,
  used_by_device_id TEXT REFERENCES devices(id),
  attempts    INTEGER NOT NULL DEFAULT 0     -- lock after 5
);
```

## 2. Storage and files

```sql
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
  rel_path      TEXT NOT NULL,               -- path relative to root, forward slashes
  name          TEXT NOT NULL,
  category      TEXT NOT NULL CHECK (category IN ('photo','video','document','music','download','backup','other')),
  mime          TEXT,
  size          INTEGER NOT NULL,
  hash          TEXT NOT NULL,               -- BLAKE3 root
  chunk_size    INTEGER NOT NULL DEFAULT 4194304,
  source_mode   TEXT NOT NULL DEFAULT 'upload' CHECK (source_mode IN ('upload','import','keep_in_place')),
  origin_device_id TEXT REFERENCES devices(id),
  client_item_id TEXT,                       -- device-local identifier for dedupe/idempotency
  created_at    INTEGER NOT NULL,            -- time ingested
  modified_at   INTEGER,                     -- original mtime if known
  last_verified_at INTEGER,                  -- last integrity scrub OK
  deleted_at    INTEGER,                     -- soft delete -> trash
  UNIQUE (root_id, rel_path)
);
CREATE INDEX idx_files_hash ON files(hash);
CREATE INDEX idx_files_category_created ON files(category, created_at DESC);
CREATE INDEX idx_files_device_item ON files(origin_device_id, client_item_id);
CREATE INDEX idx_files_deleted ON files(deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE file_chunks (                   -- optional manifest (kept for large files / download verify)
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
```

## 3. Transfers

```sql
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
  target_device_id TEXT REFERENCES devices(id),  -- for relay (phone->mac)
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  completed_at  INTEGER,
  expires_at    INTEGER NOT NULL             -- open sessions GC'd after 7 days idle
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
```

## 4. Photo backup and gallery

```sql
CREATE TABLE backup_sources (                -- one per (device, source kind)
  id          TEXT PRIMARY KEY,
  device_id   TEXT NOT NULL REFERENCES devices(id),
  kind        TEXT NOT NULL CHECK (kind IN ('camera_roll','folder')),
  label       TEXT,
  enabled     INTEGER NOT NULL DEFAULT 0,    -- set after one-time approval
  approved_at INTEGER,
  wifi_only   INTEGER NOT NULL DEFAULT 1,
  charging_only INTEGER NOT NULL DEFAULT 0,
  last_run_at INTEGER
);

CREATE TABLE backup_items (
  id            TEXT PRIMARY KEY,
  source_id     TEXT NOT NULL REFERENCES backup_sources(id) ON DELETE CASCADE,
  client_item_id TEXT NOT NULL,              -- MediaStore ID / PHAsset localIdentifier
  file_id       TEXT REFERENCES files(id),
  hash          TEXT,
  status        TEXT NOT NULL CHECK (status IN ('pending','uploading','verified','failed','skipped')),
  verified_at   INTEGER,
  local_freed_at INTEGER,                    -- set when client reports local copy removed
  UNIQUE (source_id, client_item_id)
);
CREATE INDEX idx_backup_items_status ON backup_items(status);

CREATE TABLE media (                         -- photo/video metadata, 1:1 with files
  file_id     TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
  type        TEXT NOT NULL CHECK (type IN ('photo','video')),
  taken_at    INTEGER NOT NULL,              -- EXIF DateTimeOriginal else mtime
  taken_at_source TEXT NOT NULL CHECK (taken_at_source IN ('exif','container','mtime','upload')),
  width       INTEGER,
  height      INTEGER,
  duration_ms INTEGER,
  orientation INTEGER,
  camera_make TEXT,
  camera_model TEXT,
  gps_lat     REAL,
  gps_lon     REAL,
  phash       INTEGER,                       -- 64-bit perceptual hash (Phase 2)
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
```

## 5. Duplicates

```sql
CREATE TABLE duplicate_groups (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('exact','likely','similar')),
  hash        TEXT,                          -- for exact
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
```

## 6. Disk health and integrity

```sql
CREATE TABLE disks (
  id          TEXT PRIMARY KEY,              -- stable id (serial or WMI id)
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
```

## 7. Import of existing Windows data

```sql
CREATE TABLE import_scans (
  id TEXT PRIMARY KEY, started_at INTEGER NOT NULL, finished_at INTEGER,
  roots_json TEXT NOT NULL,                  -- scanned folders
  summary_json TEXT                          -- {photos:{count,bytes}, videos:..., documents:..., downloads:..., other:...}
);
CREATE TABLE import_decisions (
  scan_id  TEXT NOT NULL REFERENCES import_scans(id) ON DELETE CASCADE,
  category TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN ('keep','import','later')),
  decided_at INTEGER NOT NULL,
  PRIMARY KEY (scan_id, category)
) WITHOUT ROWID;
```

## 8. Network, hardware, remote, alerts

```sql
CREATE TABLE hub_network (
  id TEXT PRIMARY KEY,
  iface_name TEXT, mac_address TEXT, kind TEXT CHECK (kind IN ('ethernet','wifi','hotspot')),
  link_speed_mbps INTEGER, wifi_standard TEXT, last_ip TEXT, updated_at INTEGER NOT NULL
);

CREATE TABLE hardware_audit (
  id TEXT PRIMARY KEY,
  taken_at INTEGER NOT NULL,
  report_json TEXT NOT NULL,                 -- raw collected data
  rating_storage TEXT, rating_photo_backup TEXT, rating_file_sharing TEXT,
  rating_streaming TEXT, rating_local_ai TEXT,
  supported_status TEXT CHECK (supported_status IN ('supported','untested','unsupported'))
);

CREATE TABLE remote_sessions (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL REFERENCES devices(id),
  kind TEXT NOT NULL CHECK (kind IN ('input','screen_view','screen_cast')),
  started_at INTEGER NOT NULL, ended_at INTEGER
);

CREATE TABLE alerts (
  id TEXT PRIMARY KEY,
  severity TEXT NOT NULL CHECK (severity IN ('info','warning','critical')),
  code TEXT NOT NULL,                        -- DISK_FAILING, LOW_SPACE, NO_SECOND_COPY, INTEGRITY_MISMATCH
  message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  acknowledged_at INTEGER,
  resolved_at INTEGER
);

CREATE TABLE audit_log (                     -- security-relevant events, append-only
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts INTEGER NOT NULL,
  device_id TEXT,
  action TEXT NOT NULL,                      -- pair, revoke, auth_fail, remote_start, power_action, file_delete, ...
  detail TEXT,
  ip TEXT
);
CREATE INDEX idx_audit_ts ON audit_log(ts DESC);
```

---

## 9. Client-side schema (Android Room / Swift) — upload queue

```sql
CREATE TABLE queue_items (
  id TEXT PRIMARY KEY,
  hub_id TEXT NOT NULL,
  source_uri TEXT NOT NULL,                  -- content:// or file URL
  client_item_id TEXT NOT NULL,
  name TEXT NOT NULL, size INTEGER NOT NULL, mime TEXT,
  kind TEXT NOT NULL,                        -- send | backup
  root_hash TEXT,                            -- computed lazily
  source_mtime INTEGER,
  transfer_id TEXT,                          -- from Hub once created
  state TEXT NOT NULL,                       -- queued|connecting|uploading|verifying|done|failed_retry|failed_perm
  attempts INTEGER NOT NULL DEFAULT 0,
  next_attempt_at INTEGER,
  last_error TEXT,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX idx_queue_state ON queue_items(state, next_attempt_at);

CREATE TABLE hub_trust (                     -- paired hubs known to this client
  hub_id TEXT PRIMARY KEY, name TEXT, ca_fingerprint TEXT NOT NULL,
  ca_cert_pem TEXT NOT NULL, last_addr TEXT, paired_at INTEGER NOT NULL
);
-- Private key lives in Android Keystore / Apple Keychain, never in DB.
```

## 10. Migrations

- Directory `hub/crates/hh-db/migrations/NNNN_description.sql`, applied in order inside a transaction; `hub.schema_version` updated.
- Forward-only. Before each migration: copy `hub.db` → `hub.db.bak-<version>` (keep last 3).
- Startup refuses to run if DB schema is **newer** than the binary.

## 11. Retention and GC

| Data | Policy |
|---|---|
| Open transfers idle >7 days | Mark `expired`, delete `.part` file |
| Completed transfers | Keep 90 days of rows |
| Health snapshots | Keep daily after 30 days, all within 30 days |
| Audit log | Keep 1 year |
| Trash | 30 days default, then purge file + row |
| Thumbnails | Regenerable; LRU evict if cache >5% of library |

## 12. Entity relationship overview

```text
hub 1─* devices 1─* transfers *─1 files 1─1 media
                  │                │
                  └─* backup_sources 1─* backup_items ─┘
files *─* duplicate_groups (via duplicate_members)
storage_roots 1─* files ; disks 1─* disk_health_snapshots
```
