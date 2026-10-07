//! Backup sources, diff and free-space bookkeeping (API_SPEC §7, FR-5.x).

use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{db_e, PhotoService};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSource {
    pub id: String,
    pub device_id: String,
    pub kind: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub wifi_only: bool,
    pub charging_only: bool,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItem {
    pub client_item_id: String,
    pub size: u64,
    pub taken_at: Option<i64>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffResponse {
    pub needed: Vec<String>,
    pub already_backed_up: u64,
    pub new_photos: u64,
    pub new_videos: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupSummary {
    pub backed_up: u64,
    pub pending: u64,
    pub failed: u64,
    pub last_run_at: Option<i64>,
    pub reclaimable_bytes: u64, // verified items not yet freed locally
}

impl PhotoService {
    /// Register/approve a backup source (FR-5.1 one-time approval).
    pub fn upsert_source(&self, device_id: &str, kind: &str, label: Option<&str>) -> Result<BackupSource> {
        let existing: Option<String> = {
            let c = self.db.lock()?;
            c.query_row(
                "SELECT id FROM backup_sources WHERE device_id=?1 AND kind=?2",
                params![device_id, kind],
                |r| r.get(0),
            )
            .ok()
        };
        let id = existing.unwrap_or_else(|| ulid::Ulid::new().to_string());
        let c = self.db.lock()?;
        c.execute(
            "INSERT INTO backup_sources (id, device_id, kind, label, enabled, approved_at)
             VALUES (?1,?2,?3,?4,1,?5)
             ON CONFLICT(id) DO UPDATE SET enabled=1, approved_at=excluded.approved_at",
            params![id, device_id, kind, label, now_ms()],
        )
        .map_err(db_e)?;
        drop(c);
        self.get_source(&id)
    }

    pub fn get_source(&self, id: &str) -> Result<BackupSource> {
        let c = self.db.lock()?;
        c.query_row(
            "SELECT id, device_id, kind, label, enabled, wifi_only, charging_only, last_run_at
             FROM backup_sources WHERE id=?1",
            params![id],
            |r| {
                Ok(BackupSource {
                    id: r.get(0)?,
                    device_id: r.get(1)?,
                    kind: r.get(2)?,
                    label: r.get(3)?,
                    enabled: r.get::<_, i64>(4)? == 1,
                    wifi_only: r.get::<_, i64>(5)? == 1,
                    charging_only: r.get::<_, i64>(6)? == 1,
                    last_run_at: r.get(7)?,
                })
            },
        )
        .map_err(|_| Error::NotFound(format!("backup source {id}")))
    }

    pub fn update_source(&self, id: &str, enabled: Option<bool>, wifi_only: Option<bool>, charging_only: Option<bool>) -> Result<()> {
        let c = self.db.lock()?;
        if let Some(e) = enabled {
            c.execute("UPDATE backup_sources SET enabled=?2 WHERE id=?1", params![id, e as i64])
                .map_err(db_e)?;
        }
        if let Some(w) = wifi_only {
            c.execute("UPDATE backup_sources SET wifi_only=?2 WHERE id=?1", params![id, w as i64])
                .map_err(db_e)?;
        }
        if let Some(ch) = charging_only {
            c.execute("UPDATE backup_sources SET charging_only=?2 WHERE id=?1", params![id, ch as i64])
                .map_err(db_e)?;
        }
        Ok(())
    }

    /// Diff the device's camera roll against hub state (API_SPEC §7).
    pub fn diff(&self, source_id: &str, items: &[DiffItem]) -> Result<DiffResponse> {
        let mut needed = Vec::new();
        let mut already = 0u64;
        {
            let mut c = self.db.lock()?;
            let tx = c.transaction().map_err(db_e)?;
            for item in items {
                let row: Option<(String, Option<String>)> = tx
                    .query_row(
                        "SELECT status, hash FROM backup_items WHERE source_id=?1 AND client_item_id=?2",
                        params![source_id, item.client_item_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();
                let needs_upload = match row {
                    Some((status, _)) if status == "verified" => false,
                    _ => true,
                };
                // Cross-source dedupe: identical hash already stored → mark verified
                // without re-upload (PB-03 resume without duplicates).
                if needs_upload {
                    if let Some(hash) = &item.hash {
                        let dup: Option<String> = tx
                            .query_row(
                                "SELECT id FROM files WHERE hash=?1 AND deleted_at IS NULL LIMIT 1",
                                params![hash],
                                |r| r.get(0),
                            )
                            .ok();
                        if let Some(file_id) = dup {
                            tx.execute(
                                "INSERT INTO backup_items (id, source_id, client_item_id, file_id, hash, status, verified_at)
                                 VALUES (?1,?2,?3,?4,?5,'verified',?6)
                                 ON CONFLICT(source_id, client_item_id)
                                 DO UPDATE SET status='verified', file_id=excluded.file_id, hash=excluded.hash, verified_at=excluded.verified_at",
                                params![ulid::Ulid::new().to_string(), source_id, item.client_item_id, file_id, hash, now_ms()],
                            )
                            .map_err(db_e)?;
                            already += 1;
                            continue;
                        }
                    }
                    tx.execute(
                        "INSERT INTO backup_items (id, source_id, client_item_id, hash, status)
                         VALUES (?1,?2,?3,?4,'pending')
                         ON CONFLICT(source_id, client_item_id) DO NOTHING",
                        params![ulid::Ulid::new().to_string(), source_id, item.client_item_id, item.hash],
                    )
                    .map_err(db_e)?;
                    needed.push(item.client_item_id.clone());
                } else {
                    already += 1;
                }
            }
            tx.execute("UPDATE backup_sources SET last_run_at=?2 WHERE id=?1", params![source_id, now_ms()])
                .map_err(db_e)?;
            tx.commit().map_err(db_e)?;
        }
        Ok(DiffResponse {
            new_photos: needed.len() as u64, // refined client-side by mime
            new_videos: 0,
            already_backed_up: already,
            needed,
        })
    }

    /// Mark an item verified after its transfer completed (called by the
    /// transfer-complete hook when kind = backup).
    pub fn mark_item_verified(&self, source_id: &str, client_item_id: &str, file_id: &str, hash: &str) -> Result<()> {
        let c = self.db.lock()?;
        c.execute(
            "INSERT INTO backup_items (id, source_id, client_item_id, file_id, hash, status, verified_at)
             VALUES (?1,?2,?3,?4,?5,'verified',?6)
             ON CONFLICT(source_id, client_item_id)
             DO UPDATE SET status='verified', file_id=excluded.file_id, hash=excluded.hash, verified_at=excluded.verified_at",
            params![ulid::Ulid::new().to_string(), source_id, client_item_id, file_id, hash, now_ms()],
        )
        .map_err(db_e)?;
        Ok(())
    }

    /// Client reports the local copy was removed (FR-5.3 bookkeeping).
    pub fn confirm_local_freed(&self, source_id: &str, client_item_id: &str) -> Result<()> {
        let c = self.db.lock()?;
        let status: Option<String> = c
            .query_row(
                "SELECT status FROM backup_items WHERE source_id=?1 AND client_item_id=?2",
                params![source_id, client_item_id],
                |r| r.get(0),
            )
            .ok();
        if status.as_deref() != Some("verified") {
            return Err(Error::Conflict("item is not verified on the Hub; refusing to confirm free".into()));
        }
        c.execute(
            "UPDATE backup_items SET local_freed_at=?3 WHERE source_id=?1 AND client_item_id=?2",
            params![source_id, client_item_id, now_ms()],
        )
        .map_err(db_e)?;
        Ok(())
    }

    pub fn summary(&self, source_id: &str) -> Result<BackupSummary> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT status, COUNT(*), COALESCE(SUM(f.size),0)
                      FROM backup_items b LEFT JOIN files f ON f.id = b.file_id
                      WHERE b.source_id=?1 GROUP BY status")
            .map_err(db_e)?;
        let rows: Vec<(String, i64, i64)> = st
            .query_map(params![source_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        let last_run: Option<i64> = c
            .query_row("SELECT last_run_at FROM backup_sources WHERE id=?1", params![source_id], |r| r.get(0))
            .ok()
            .flatten();
        let mut s = BackupSummary { backed_up: 0, pending: 0, failed: 0, last_run_at: last_run, reclaimable_bytes: 0 };
        for (status, count, bytes) in rows {
            match status.as_str() {
                "verified" => {
                    s.backed_up = count as u64;
                    s.reclaimable_bytes = bytes as u64;
                }
                "pending" | "uploading" => s.pending += count as u64,
                "failed" => s.failed = count as u64,
                _ => {}
            }
        }
        Ok(s)
    }
}
