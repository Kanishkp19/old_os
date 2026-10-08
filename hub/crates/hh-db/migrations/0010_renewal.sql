CREATE TABLE certificate_renewals (
 device_id TEXT PRIMARY KEY REFERENCES devices(id),
 old_serial TEXT NOT NULL,
 new_serial TEXT NOT NULL UNIQUE,
 cert_pem TEXT NOT NULL,
 csr_pem TEXT NOT NULL,
 cert_expires_at INTEGER NOT NULL,
 expires_at INTEGER NOT NULL
);
