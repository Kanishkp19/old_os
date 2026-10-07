-- 0006: import, network, hardware, remote, alerts, audit (BACKEND_SCHEMA §7, §8)
CREATE TABLE import_scans (
  id TEXT PRIMARY KEY, started_at INTEGER NOT NULL, finished_at INTEGER,
  roots_json TEXT NOT NULL,
  summary_json TEXT
);
CREATE TABLE import_decisions (
  scan_id  TEXT NOT NULL REFERENCES import_scans(id) ON DELETE CASCADE,
  category TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN ('keep','import','later')),
  decided_at INTEGER NOT NULL,
  PRIMARY KEY (scan_id, category)
) WITHOUT ROWID;

CREATE TABLE hub_network (
  id TEXT PRIMARY KEY,
  iface_name TEXT, mac_address TEXT, kind TEXT CHECK (kind IN ('ethernet','wifi','hotspot')),
  link_speed_mbps INTEGER, wifi_standard TEXT, last_ip TEXT, updated_at INTEGER NOT NULL
);

CREATE TABLE hardware_audit (
  id TEXT PRIMARY KEY,
  taken_at INTEGER NOT NULL,
  report_json TEXT NOT NULL,
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
  code TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  acknowledged_at INTEGER,
  resolved_at INTEGER
);

CREATE TABLE audit_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts INTEGER NOT NULL,
  device_id TEXT,
  action TEXT NOT NULL,
  detail TEXT,
  ip TEXT
);
CREATE INDEX idx_audit_ts ON audit_log(ts DESC);
