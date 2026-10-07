-- 0001: core identity (BACKEND_SCHEMA §1)
CREATE TABLE hub (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  ca_cert_pem   TEXT NOT NULL,
  ca_key_ref    TEXT NOT NULL,
  server_cert_pem TEXT NOT NULL,
  schema_version INTEGER NOT NULL
);

CREATE TABLE settings (
  key     TEXT PRIMARY KEY,
  value   TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE devices (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  platform      TEXT NOT NULL CHECK (platform IN ('android','ios','macos','windows','linux','web')),
  model         TEXT,
  app_version   TEXT,
  public_key    TEXT NOT NULL,
  cert_pem      TEXT NOT NULL,
  cert_serial   TEXT NOT NULL UNIQUE,
  cert_expires_at INTEGER NOT NULL,
  scopes        TEXT NOT NULL DEFAULT 'files,transfer,photos',
  mac_address   TEXT,
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
  token_hash  TEXT NOT NULL UNIQUE,
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL,
  used_at     INTEGER,
  used_by_device_id TEXT REFERENCES devices(id),
  attempts    INTEGER NOT NULL DEFAULT 0
);
