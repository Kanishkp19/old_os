//! Typed queries shared by hub crates. Write transactions use
//! `BEGIN IMMEDIATE` via rusqlite's `transaction()` on the single writer
//! connection (AGENTS.md §4).

use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use hh_core::types::{ChunkBitmap, Device, TransferStatus};
use rusqlite::params;
use rusqlite::OptionalExtension;

use crate::{db_err, opt, Db};

// ---- settings ----

impl Db {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let c = self.lock()?;
        opt(c.query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0)))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            params![key, value, now_ms()],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn hub_identity(&self) -> Result<(String, String)> {
        let c = self.lock()?;
        c.query_row("SELECT id, name FROM hub LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db_err)
    }

    pub fn set_hub_name(&self, name: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute("UPDATE hub SET name=?1", params![name]).map_err(db_err)?;
        Ok(())
    }
}

// ---- devices ----

#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub model: Option<String>,
    pub app_version: Option<String>,
    pub cert_serial: String,
    pub cert_expires_at: i64,
    pub scopes: Vec<String>,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
    pub status: String,
}

impl DeviceRow {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
    pub fn to_api(&self) -> Device {
        Device {
            id: self.id.clone(),
            name: self.name.clone(),
            platform: self.platform.clone(),
            model: self.model.clone(),
            app_version: self.app_version.clone(),
            scopes: self.scopes.clone(),
            paired_at: self.paired_at,
            last_seen_at: self.last_seen_at,
            status: self.status.clone(),
        }
    }
}

impl Db {
    pub fn insert_device(&self, d: &DeviceRow, cert_pem: &str, public_key: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO devices (id, name, platform, model, app_version, public_key, cert_pem,
                cert_serial, cert_expires_at, scopes, paired_at, status)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'active')",
            params![
                d.id, d.name, d.platform, d.model, d.app_version, public_key, cert_pem,
                d.cert_serial, d.cert_expires_at, d.scopes.join(","), d.paired_at
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn device_by_id(&self, id: &str) -> Result<Option<DeviceRow>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT id,name,platform,model,app_version,cert_serial,cert_expires_at,scopes,paired_at,last_seen_at,status
             FROM devices WHERE id=?1",
            params![id],
            |r| Ok(device_row(r)?),
        ))
    }

    pub fn device_by_serial(&self, serial: &str) -> Result<Option<DeviceRow>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT id,name,platform,model,app_version,cert_serial,cert_expires_at,scopes,paired_at,last_seen_at,status
             FROM devices WHERE cert_serial=?1",
            params![serial],
            |r| Ok(device_row(r)?),
        ))
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceRow>> {
        let c = self.lock()?;
        let mut st = c
            .prepare(
                "SELECT id,name,platform,model,app_version,cert_serial,cert_expires_at,scopes,paired_at,last_seen_at,status
                 FROM devices ORDER BY paired_at",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| Ok(device_row(r)?))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// Revocation is immediate: status flip + revocation row, same tx (FR-2.5).
    pub fn revoke_device(&self, id: &str, reason: &str) -> Result<()> {
        let mut c = self.lock()?;
        let tx = c.transaction().map_err(db_err)?;
        let serial: Option<String> = tx
            .query_row("SELECT cert_serial FROM devices WHERE id=?1", params![id], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        let serial = serial.ok_or_else(|| Error::NotFound(format!("device {id}")))?;
        tx.execute(
            "UPDATE devices SET status='revoked', revoked_at=?2 WHERE id=?1",
            params![id, now_ms()],
        )
        .map_err(db_err)?;
        tx.execute(
            "INSERT OR REPLACE INTO revoked_certs (cert_serial, device_id, revoked_at, reason)
             VALUES (?1,?2,?3,?4)",
            params![serial, id, now_ms(), reason],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn is_serial_revoked(&self, serial: &str) -> Result<bool> {
        let c = self.lock()?;
        let n: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM revoked_certs WHERE cert_serial=?1",
                params![serial],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        Ok(n > 0)
    }

    pub fn revoked_serials(&self) -> Result<Vec<String>> {
        let c = self.lock()?;
        let mut st = c.prepare("SELECT cert_serial FROM revoked_certs").map_err(db_err)?;
        let rows = st
            .query_map([], |r| r.get(0))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn touch_device_seen(&self, id: &str, ip: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "UPDATE devices SET last_seen_at=?2, last_ip=?3 WHERE id=?1",
            params![id, now_ms(), ip],
        )
        .map_err(db_err)?;
        Ok(())
    }

    // ---- pairing tokens (SECURITY §5: stored hashed, single-use, 5-attempt lockout) ----

    pub fn insert_pairing_token(&self, id: &str, token_hash: &str, expires_at: i64) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO pairing_tokens (id, token_hash, created_at, expires_at) VALUES (?1,?2,?3,?4)",
            params![id, token_hash, now_ms(), expires_at],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn get_pairing_token(&self, token_hash: &str) -> Result<Option<(String, i64, Option<i64>, i64)>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT id, expires_at, used_at, attempts FROM pairing_tokens WHERE token_hash=?1",
            params![token_hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ))
    }

    pub fn bump_token_attempts(&self, id: &str) -> Result<i64> {
        let c = self.lock()?;
        c.execute(
            "UPDATE pairing_tokens SET attempts = attempts + 1 WHERE id=?1",
            params![id],
        )
        .map_err(db_err)?;
        c.query_row("SELECT attempts FROM pairing_tokens WHERE id=?1", params![id], |r| r.get(0))
            .map_err(db_err)
    }

    pub fn burn_token(&self, id: &str, device_id: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "UPDATE pairing_tokens SET used_at=?2, used_by_device_id=?3 WHERE id=?1",
            params![id, now_ms(), device_id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    // ---- transfers ----

    #[allow(clippy::too_many_arguments)]
    pub fn insert_transfer(
        &self,
        id: &str,
        device_id: &str,
        kind: &str,
        name: &str,
        size: u64,
        mime: Option<&str>,
        rel_path: Option<&str>,
        chunk_size: u64,
        chunk_count: u64,
        expected_root_hash: Option<&str>,
        client_item_id: Option<&str>,
        tmp_path: &str,
        expires_at: i64,
    ) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO transfers (id, device_id, direction, kind, name, size, mime, rel_path,
                chunk_size, chunk_count, expected_root_hash, client_item_id, tmp_path, status,
                created_at, updated_at, expires_at)
             VALUES (?1,?2,'upload',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'open',?13,?13,?14)",
            params![
                id, device_id, kind, name, size as i64, mime, rel_path, chunk_size as i64,
                chunk_count as i64, expected_root_hash, client_item_id, tmp_path, now_ms(),
                expires_at
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn get_transfer(&self, id: &str) -> Result<Option<TransferRow>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT id,device_id,kind,name,size,mime,rel_path,chunk_size,chunk_count,
                    expected_root_hash,client_item_id,tmp_path,status,bytes_verified,
                    created_at,updated_at,result_file_id,error_code
             FROM transfers WHERE id=?1",
            params![id],
            |r| transfer_row(r),
        ))
    }

    pub fn find_resumable(&self, device_id: &str, client_item_id: &str, size: u64) -> Result<Option<TransferRow>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT id,device_id,kind,name,size,mime,rel_path,chunk_size,chunk_count,
                    expected_root_hash,client_item_id,tmp_path,status,bytes_verified,
                    created_at,updated_at,result_file_id,error_code
             FROM transfers
             WHERE device_id=?1 AND client_item_id=?2 AND size=?3 AND status='open'
             ORDER BY updated_at DESC LIMIT 1",
            params![device_id, client_item_id, size as i64],
            |r| transfer_row(r),
        ))
    }

    pub fn update_transfer_status(&self, id: &str, status: &str, error: Option<&str>) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "UPDATE transfers SET status=?2, error_code=?3, updated_at=?4,
                completed_at = CASE WHEN ?2 IN ('completed','failed','aborted') THEN ?4 ELSE completed_at END
             WHERE id=?1",
            params![id, status, error, now_ms()],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn set_transfer_result(&self, id: &str, file_id: &str) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "UPDATE transfers SET result_file_id=?2, updated_at=?3 WHERE id=?1",
            params![id, file_id, now_ms()],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn list_transfers(&self, device_id: Option<&str>, status: Option<&str>) -> Result<Vec<TransferRow>> {
        let c = self.lock()?;
        let mut sql = String::from(
            "SELECT id,device_id,kind,name,size,mime,rel_path,chunk_size,chunk_count,
                    expected_root_hash,client_item_id,tmp_path,status,bytes_verified,
                    created_at,updated_at,result_file_id,error_code FROM transfers WHERE 1=1",
        );
        let mut vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        if let Some(d) = device_id {
            sql.push_str(" AND device_id=?");
            vals.push(Box::new(d.to_string()));
        }
        if let Some(s) = status {
            sql.push_str(" AND status=?");
            vals.push(Box::new(s.to_string()));
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT 500");
        let mut st = c.prepare(&sql).map_err(db_err)?;
        let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
        let rows = st
            .query_map(refs.as_slice(), |r| transfer_row(r))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    // ---- transfer chunks ----

    pub fn record_chunk(&self, transfer_id: &str, idx: u64, hash: &str, size: u64) -> Result<()> {
        let mut c = self.lock()?;
        let tx = c.transaction().map_err(db_err)?;
        tx.execute(
            "INSERT OR REPLACE INTO transfer_chunks (transfer_id, idx, hash, size, verified_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![transfer_id, idx as i64, hash, size as i64, now_ms()],
        )
        .map_err(db_err)?;
        tx.execute(
            "UPDATE transfers SET bytes_verified = (SELECT COALESCE(SUM(size),0) FROM transfer_chunks WHERE transfer_id=?1),
                    updated_at=?2 WHERE id=?1",
            params![transfer_id, now_ms()],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn chunk_bitmap(&self, transfer_id: &str) -> Result<ChunkBitmap> {
        let c = self.lock()?;
        let mut st = c
            .prepare("SELECT idx FROM transfer_chunks WHERE transfer_id=?1 ORDER BY idx")
            .map_err(db_err)?;
        let idxs: Vec<u64> = st
            .query_map(params![transfer_id], |r| r.get::<_, i64>(0))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<i64>, _>>()
            .map_err(db_err)?
            .into_iter()
            .map(|i| i as u64)
            .collect();
        Ok(ChunkBitmap::from_chunks(idxs))
    }

    pub fn chunk_hash(&self, transfer_id: &str, idx: u64) -> Result<Option<String>> {
        let c = self.lock()?;
        opt(c.query_row(
            "SELECT hash FROM transfer_chunks WHERE transfer_id=?1 AND idx=?2",
            params![transfer_id, idx as i64],
            |r| r.get(0),
        ))
    }

    pub fn expire_idle_transfers(&self, idle_ttl_ms: i64) -> Result<Vec<String>> {
        let cutoff = now_ms() - idle_ttl_ms;
        let c = self.lock()?;
        let mut st = c
            .prepare("SELECT id, tmp_path FROM transfers WHERE status='open' AND updated_at < ?1")
            .map_err(db_err)?;
        let rows: Vec<(String, Option<String>)> = st
            .query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        drop(st);
        for (id, _) in &rows {
            c.execute(
                "UPDATE transfers SET status='expired', updated_at=?2 WHERE id=?1",
                params![id, now_ms()],
            )
            .map_err(db_err)?;
        }
        Ok(rows.into_iter().filter_map(|(_, p)| p).collect())
    }

    // ---- audit log (append-only, BACKEND_SCHEMA §8) ----

    pub fn audit(&self, device_id: Option<&str>, action: &str, detail: Option<&str>, ip: Option<&str>) -> Result<()> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO audit_log (ts, device_id, action, detail, ip) VALUES (?1,?2,?3,?4,?5)",
            params![now_ms(), device_id, action, detail, ip],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// Newest-first audit page for the dashboard Activity view (FR-2.6).
    /// Returns (rows, total_matching).
    pub fn list_audit(&self, device_id: Option<&str>, limit: u32, offset: u32) -> Result<(Vec<AuditRow>, i64)> {
        let limit = limit.clamp(1, 500);
        let c = self.lock()?;
        let total: i64 = if let Some(d) = device_id {
            c.query_row("SELECT COUNT(*) FROM audit_log WHERE device_id=?1", params![d], |r| r.get(0))
        } else {
            c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
        }
        .map_err(db_err)?;
        let mut sql = String::from(
            "SELECT id, ts, device_id, action, detail, ip FROM audit_log WHERE 1=1",
        );
        let mut vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        if let Some(d) = device_id {
            sql.push_str(" AND device_id=?");
            vals.push(Box::new(d.to_string()));
        }
        sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
        vals.push(Box::new(limit as i64));
        vals.push(Box::new(offset as i64));
        let mut st = c.prepare(&sql).map_err(db_err)?;
        let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
        let rows = st
            .query_map(refs.as_slice(), |r| {
                Ok(AuditRow {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    device_id: r.get(2)?,
                    action: r.get(3)?,
                    detail: r.get(4)?,
                    ip: r.get(5)?,
                })
            })
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok((rows, total))
    }

    /// Verified-but-not-yet-freed backup items for the phone's
    /// "Free phone storage" flow (FR-5.3, W1.3).
    pub fn backup_verified_items(&self, source_id: &str, limit: u32) -> Result<Vec<VerifiedItemRow>> {
        let c = self.lock()?;
        let mut st = c
            .prepare(
                "SELECT b.client_item_id, b.file_id, b.verified_at, COALESCE(f.size, 0)
                 FROM backup_items b LEFT JOIN files f ON f.id = b.file_id
                 WHERE b.source_id=?1 AND b.status='verified' AND b.local_freed_at IS NULL
                 ORDER BY b.verified_at DESC LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map(params![source_id, limit.clamp(1, 1000) as i64], |r| {
                Ok(VerifiedItemRow {
                    client_item_id: r.get(0)?,
                    file_id: r.get(1)?,
                    verified_at: r.get(2)?,
                    size: r.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// Backup sources owned by one device (the phone lists its own sources
    /// before opening the free-storage sheet).
    pub fn backup_sources_for_device(&self, device_id: &str) -> Result<Vec<(String, String)>> {
        let c = self.lock()?;
        let mut st = c
            .prepare("SELECT id, COALESCE(label, kind) FROM backup_sources WHERE device_id=?1 ORDER BY approved_at")
            .map_err(db_err)?;
        let rows = st
            .query_map(params![device_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }
}

    // ---- alerts ----

    pub fn create_alert(&self, severity: &str, code: &str, message: &str) -> Result<String> {
        let id = ulid::Ulid::new().to_string();
        let c = self.lock()?;
        c.execute(
            "INSERT INTO alerts (id, severity, code, message, created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, severity, code, message, now_ms()],
        )
        .map_err(db_err)?;
        Ok(id)
    }

    pub fn active_alert_count(&self) -> Result<u32> {
        let c = self.lock()?;
        let n: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM alerts WHERE resolved_at IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        Ok(n as u32)
    }
}

/// One audit-log row for the dashboard Activity view.
#[derive(Debug, Clone)]
pub struct AuditRow {
    pub id: i64,
    pub ts: i64,
    pub device_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub ip: Option<String>,
}

/// A verified backup item that still exists on the phone (FR-5.3).
#[derive(Debug, Clone)]
pub struct VerifiedItemRow {
    pub client_item_id: String,
    pub file_id: Option<String>,
    pub verified_at: Option<i64>,
    pub size: u64,
}

fn device_row(r: &rusqlite::Row<'_>) -> std::result::Result<DeviceRow, rusqlite::Error> {
    let scopes_csv: String = r.get(7)?;
    Ok(DeviceRow {
        id: r.get(0)?,
        name: r.get(1)?,
        platform: r.get(2)?,
        model: r.get(3)?,
        app_version: r.get(4)?,
        cert_serial: r.get(5)?,
        cert_expires_at: r.get(6)?,
        scopes: scopes_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        paired_at: r.get(8)?,
        last_seen_at: r.get(9)?,
        status: r.get(10)?,
    })
}

#[derive(Debug, Clone)]
pub struct TransferRow {
    pub id: String,
    pub device_id: String,
    pub kind: String,
    pub name: String,
    pub size: u64,
    pub mime: Option<String>,
    pub rel_path: Option<String>,
    pub chunk_size: u64,
    pub chunk_count: u64,
    pub expected_root_hash: Option<String>,
    pub client_item_id: Option<String>,
    pub tmp_path: Option<String>,
    pub status: String,
    pub bytes_verified: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub result_file_id: Option<String>,
    pub error_code: Option<String>,
}

impl TransferRow {
    pub fn to_status(&self, have: ChunkBitmap) -> TransferStatus {
        TransferStatus {
            transfer_id: self.id.clone(),
            status: self.status.clone(),
            name: self.name.clone(),
            size: self.size,
            bytes_verified: self.bytes_verified,
            chunk_size: self.chunk_size,
            chunk_count: self.chunk_count,
            have,
            invalid_chunks: vec![],
            error_code: self.error_code.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

fn transfer_row(r: &rusqlite::Row<'_>) -> std::result::Result<TransferRow, rusqlite::Error> {
    Ok(TransferRow {
        id: r.get(0)?,
        device_id: r.get(1)?,
        kind: r.get(2)?,
        name: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        mime: r.get(5)?,
        rel_path: r.get(6)?,
        chunk_size: r.get::<_, i64>(7)? as u64,
        chunk_count: r.get::<_, i64>(8)? as u64,
        expected_root_hash: r.get(9)?,
        client_item_id: r.get(10)?,
        tmp_path: r.get(11)?,
        status: r.get(12)?,
        bytes_verified: r.get::<_, i64>(13)? as u64,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
        result_file_id: r.get(16)?,
        error_code: r.get(17)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_to_memory_db() {
        let db = Db::open_memory().unwrap();
        let (id, _name) = db.hub_identity().unwrap();
        assert!(!id.is_empty());
        // re-opening runs migrations idempotently
        db.migrate().unwrap();
    }

    #[test]
    fn device_lifecycle() {
        let db = Db::open_memory().unwrap();
        let d = DeviceRow {
            id: "d1".into(), name: "Pixel".into(), platform: "android".into(),
            model: None, app_version: None, cert_serial: "serial-1".into(),
            cert_expires_at: now_ms() + 1000, scopes: vec!["files".into(), "transfer".into()],
            paired_at: now_ms(), last_seen_at: None, status: "active".into(),
        };
        db.insert_device(&d, "PEM", "PUB").unwrap();
        assert!(db.device_by_serial("serial-1").unwrap().is_some());
        db.revoke_device("d1", "test").unwrap();
        assert!(db.is_serial_revoked("serial-1").unwrap());
        assert_eq!(db.device_by_id("d1").unwrap().unwrap().status, "revoked");
    }

    #[test]
    fn transfer_chunk_bitmap() {
        let db = Db::open_memory().unwrap();
        let d = DeviceRow {
            id: "d1".into(), name: "n".into(), platform: "android".into(),
            model: None, app_version: None, cert_serial: "s".into(),
            cert_expires_at: 0, scopes: vec![], paired_at: 0, last_seen_at: None,
            status: "active".into(),
        };
        db.insert_device(&d, "PEM", "PUB").unwrap();
        db.insert_transfer("t1", "d1", "send", "f.bin", 100, None, None, 4, 25,
            None, None, "/tmp/x.part", now_ms() + 1000).unwrap();
        db.record_chunk("t1", 0, "h0", 4).unwrap();
        db.record_chunk("t1", 1, "h1", 4).unwrap();
        db.record_chunk("t1", 3, "h3", 4).unwrap();
        let bm = db.chunk_bitmap("t1").unwrap();
        assert_eq!(bm.ranges, vec![(0, 1), (3, 3)]);
        let t = db.get_transfer("t1").unwrap().unwrap();
        assert_eq!(t.bytes_verified, 12);
    }
}
