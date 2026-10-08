//! Backup sources, diff and free-space bookkeeping (API_SPEC §7, FR-5.x).

use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use rusqlite::{params,OptionalExtension};
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

    /// Discovery never treats a previously verified row as current unless
    /// the current source hash and the freshly read Hub content both match.
    pub fn diff(&self, source_id: &str, items: &[DiffItem]) -> Result<DiffResponse> {
        if items.len() > 2000 { return Err(Error::TooLarge("backup discovery batch".into())); }
        let source = self.get_source(source_id)?;
        if !source.enabled { return Err(Error::Conflict("backup source is disabled".into())); }
        let mut needed = Vec::new();
        let mut already = 0;
        let mut c = self.db.lock()?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_e)?;
        for item in items {
            let mut found = None;
            if let Some(hash) = &item.hash {
                let candidates: Vec<String> = {
                    let mut st = tx.prepare("SELECT id FROM files WHERE hash=?1 AND size=?2 AND deleted_at IS NULL").map_err(db_e)?;
                    let rows = st.query_map(params![hash, item.size as i64], |r| r.get(0)).map_err(db_e)?;
                    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(db_e)?
                };
                for id in candidates {
                    if verify_file(&tx, &id, hash, Some(item.size)).is_ok() { found = Some(id); break; }
                }
            }
            let status = if found.is_some() { "verified" } else { "pending" };
            tx.execute(
                "INSERT INTO backup_items(id,source_id,client_item_id,file_id,hash,status,verified_at,expected_size)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(source_id,client_item_id) DO UPDATE SET file_id=excluded.file_id,
                 hash=excluded.hash,status=excluded.status,verified_at=excluded.verified_at,
                 expected_size=excluded.expected_size,
                 local_freed_at=CASE WHEN backup_items.hash=excluded.hash THEN backup_items.local_freed_at ELSE NULL END",
                params![ulid::Ulid::new().to_string(),source_id,item.client_item_id,found,item.hash,status,
                    if status=="verified" {Some(now_ms())} else {None},item.size as i64]).map_err(db_e)?;
            if status == "verified" { already += 1; } else { needed.push(item.client_item_id.clone()); }
        }
        tx.execute("UPDATE backup_sources SET last_run_at=?2 WHERE id=?1",params![source_id,now_ms()]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(DiffResponse {new_photos:needed.len() as u64,new_videos:0,already_backed_up:already,needed})
    }

    /// Called only after transfer finalization; additionally verifies source
    /// ownership and current file bytes before setting deletion eligibility.
    pub fn mark_transfer_verified(&self, source_id:&str, device_id:&str, client_item_id:&str,
        file_id:&str, expected_hash:&str) -> Result<String> {
        let source = self.get_source(source_id)?;
        if source.device_id != device_id || !source.enabled { return Err(Error::ForbiddenScope("backup source".into())); }
        let c = self.db.lock()?;
        let expected_size: Option<i64> = c.query_row("SELECT expected_size FROM backup_items WHERE source_id=?1 AND client_item_id=?2",
            params![source_id,client_item_id],|r|r.get(0)).ok().flatten();
        verify_file(&c,file_id,expected_hash,expected_size.map(|n|n as u64))?;
        let id: String = c.query_row("SELECT id FROM backup_items WHERE source_id=?1 AND client_item_id=?2",
            params![source_id,client_item_id],|r|r.get(0)).unwrap_or_else(|_|ulid::Ulid::new().to_string());
        c.execute("INSERT INTO backup_items(id,source_id,client_item_id,file_id,hash,status,verified_at)
            VALUES(?1,?2,?3,?4,?5,'verified',?6) ON CONFLICT(source_id,client_item_id) DO UPDATE SET
            file_id=excluded.file_id,hash=excluded.hash,status='verified',verified_at=excluded.verified_at",
            params![id,source_id,client_item_id,file_id,expected_hash,now_ms()]).map_err(db_e)?;
        Ok(id)
    }

    pub fn mark_item_verified(&self, source_id:&str, client_item_id:&str, file_id:&str, hash:&str)->Result<()> {
        let source=self.get_source(source_id)?;
        self.mark_transfer_verified(source_id,&source.device_id,client_item_id,file_id,hash).map(|_|())
    }

    /// Legacy bookkeeping remains safe but does not grant deletion permission.
    pub fn confirm_local_freed(&self, source_id:&str, client_item_id:&str)->Result<()> {
        let c=self.db.lock()?;
        let row:(String,String)=c.query_row("SELECT file_id,hash FROM backup_items WHERE source_id=?1 AND client_item_id=?2 AND status='verified'",
            params![source_id,client_item_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_e)?;
        verify_file(&c,&row.0,&row.1,None)?;
        c.execute("UPDATE backup_items SET local_freed_at=?3 WHERE source_id=?1 AND client_item_id=?2",
            params![source_id,client_item_id,now_ms()]).map_err(db_e)?;
        Ok(())
    }

    pub fn create_cleanup_lease(&self,source_id:&str,device_id:&str,ids:&[String])->Result<CleanupLease> {
        self.create_cleanup_review(source_id,device_id,ids,None)
    }

    pub fn create_cleanup_review(&self,source_id:&str,device_id:&str,ids:&[String],review:Option<&str>)->Result<CleanupLease> {
        if ids.is_empty() || ids.len()>500 { return Err(Error::BadRequest("select 1–500 cleanup items".into())); }
        if review.is_some_and(|v|v.is_empty()||v.len()>128||!v.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')) {return Err(Error::BadRequest("invalid cleanup review ID".into()));}
        let source=self.get_source(source_id)?;
        if source.device_id!=device_id { return Err(Error::ForbiddenScope("backup source".into())); }
        let lease_id=ulid::Ulid::new().to_string();
        let mut c=self.db.lock()?;
        let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_e)?;
        if let Some(review)=review {
            let existing:Option<(String,String)>=tx.query_row("SELECT id,state FROM cleanup_leases WHERE source_id=?1 AND device_id=?2 AND client_review_id=?3",params![source_id,device_id,review],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_e)?;
            if let Some((id,state))=existing {if state!="active" {return Err(Error::Conflict("cleanup review completed".into()));}return read_cleanup_lease(&tx,&id);}
        }
        tx.execute("INSERT INTO cleanup_leases(id,source_id,device_id,created_at,client_review_id) VALUES(?1,?2,?3,?4,?5)",params![lease_id,source_id,device_id,now_ms(),review]).map_err(db_e)?;
        let mut items=Vec::new();
        for client_id in ids {
            let row: Option<(String,String)> = tx.query_row("SELECT file_id,hash FROM backup_items WHERE source_id=?1 AND client_item_id=?2 AND status='verified' AND local_freed_at IS NULL",
                params![source_id,client_id],|r|Ok((r.get(0)?,r.get(1)?))).ok();
            if let Some((file_id,hash))=row {
                if let Ok(size)=verify_file(&tx,&file_id,&hash,None) {
                    tx.execute("INSERT OR IGNORE INTO cleanup_lease_items(lease_id,file_id,client_item_id,hash,size) VALUES(?1,?2,?3,?4,?5)",params![lease_id,file_id,client_id,hash,size as i64]).map_err(db_e)?;
                    items.push(CleanupItem {client_item_id:client_id.clone(),file_id,hash,size});
                }
            }
        }
        tx.commit().map_err(db_e)?;
        Ok(CleanupLease{lease_id,items,client_review_id:review.map(str::to_owned)})
    }

    pub fn active_cleanup_reviews(&self,source_id:&str,device_id:&str)->Result<Vec<CleanupLease>> {
        if self.get_source(source_id)?.device_id!=device_id {return Err(Error::ForbiddenScope("backup source".into()));}
        let c=self.db.lock()?;
        let mut st=c.prepare("SELECT id FROM cleanup_leases WHERE source_id=?1 AND device_id=?2 AND state='active' ORDER BY created_at LIMIT 500").map_err(db_e)?;
        let ids=st.query_map(params![source_id,device_id],|r|r.get::<_,String>(0)).map_err(db_e)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_e)?;
        ids.iter().map(|id|read_cleanup_lease(&c,id)).collect()
    }

    pub fn finish_cleanup_lease(&self,id:&str,device_id:&str,freed:&[String])->Result<()> {
        let mut c=self.db.lock()?;
        let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_e)?;
        let (source,owner,state):(String,String,String)=tx.query_row("SELECT source_id,device_id,state FROM cleanup_leases WHERE id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_e)?;
        if owner!=device_id {return Err(Error::ForbiddenScope("cleanup lease".into()));}
        if state=="completed" {return Ok(());}
        for client_id in freed {
            let hash:String=tx.query_row("SELECT hash FROM cleanup_lease_items WHERE lease_id=?1 AND client_item_id=?2",params![id,client_id],|r|r.get(0)).map_err(|_|Error::BadRequest("item not in cleanup lease".into()))?;
            tx.execute("UPDATE backup_items SET local_freed_at=?4 WHERE source_id=?1 AND client_item_id=?2 AND hash=?3 AND status='verified'",params![source,client_id,hash,now_ms()]).map_err(db_e)?;
        }
        tx.execute("UPDATE cleanup_leases SET state='completed' WHERE id=?1",params![id]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(())
    }

    pub fn summary(&self, source_id: &str) -> Result<BackupSummary> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT status, COUNT(*), COALESCE(SUM(CASE WHEN b.local_freed_at IS NULL THEN f.size ELSE 0 END),0)
                      FROM backup_items b LEFT JOIN files f ON f.id = b.file_id
                      WHERE b.source_id=?1 AND (f.deleted_at IS NULL OR f.id IS NULL) GROUP BY status")
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

#[derive(Debug,Clone,Serialize)]
pub struct CleanupItem { pub client_item_id:String,pub file_id:String,pub hash:String,pub size:u64 }
#[derive(Debug,Clone,Serialize)]
pub struct CleanupLease { pub lease_id:String,pub items:Vec<CleanupItem>,pub client_review_id:Option<String> }

fn read_cleanup_lease(c:&rusqlite::Connection,id:&str)->Result<CleanupLease> {
    let mut st=c.prepare("SELECT client_item_id,file_id,hash,size FROM cleanup_lease_items WHERE lease_id=?1 ORDER BY client_item_id").map_err(db_e)?;
    let items=st.query_map(params![id],|r|Ok(CleanupItem{client_item_id:r.get(0)?,file_id:r.get(1)?,hash:r.get(2)?,size:r.get::<_,i64>(3)? as u64})).map_err(db_e)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_e)?;
    let review=c.query_row("SELECT client_review_id FROM cleanup_leases WHERE id=?1",params![id],|r|r.get::<_,Option<String>>(0)).map_err(db_e)?;
    Ok(CleanupLease{lease_id:id.to_owned(),items,client_review_id:review})
}

fn verify_file(c:&rusqlite::Connection,id:&str,hash:&str,size:Option<u64>)->Result<u64> {
    let (root,rel,stored_hash,stored_size):(String,String,String,i64)=c.query_row(
        "SELECT r.path,f.rel_path,f.hash,f.size FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE f.id=?1 AND f.deleted_at IS NULL AND r.is_active=1",
        params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(db_e)?;
    if !stored_hash.eq_ignore_ascii_case(hash) || size.is_some_and(|n|n!=stored_size as u64) {return Err(Error::RootHashMismatch);}
    let path=hh_core::paths::jail_join(std::path::Path::new(&root),&rel)?;
    let file=std::fs::File::open(path)?;
    if file.metadata()?.len()!=stored_size as u64 || !hh_transfer::hash_file(&file)?.eq_ignore_ascii_case(hash) {return Err(Error::RootHashMismatch);}
    Ok(stored_size as u64)
}
