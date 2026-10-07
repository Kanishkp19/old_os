//! Trash: soft delete with restore and 30-day retention purge
//! (BACKEND_SCHEMA §2/§11). Nothing here hard-deletes without retention.

use hh_core::error::Result;
use hh_core::time::now_ms;
use hh_core::types::{FileObject, Page};
use hh_core::TRASH_RETENTION_MS;
use rusqlite::params;

use crate::{db_e, StorageService};

impl StorageService {
    pub fn trash_file(&self, id: &str, by_device: Option<&str>) -> Result<()> {
        let mut c = self.db.lock()?;
        let tx = c.transaction().map_err(db_e)?;
        tx.execute("UPDATE files SET deleted_at=?2 WHERE id=?1", params![id, now_ms()])
            .map_err(db_e)?;
        tx.execute(
            "INSERT OR REPLACE INTO trash (file_id, trashed_at, trashed_by_device_id, purge_after)
             VALUES (?1,?2,?3,?4)",
            params![id, now_ms(), by_device, now_ms() + TRASH_RETENTION_MS],
        )
        .map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        self.db.audit(by_device, "file_delete", Some(id), None)?;
        Ok(())
    }

    pub fn list_trash(&self, limit: u32) -> Result<Page<FileObject>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare(
                "SELECT f.id,f.name,f.category,f.mime,f.size,f.hash,f.created_at,f.modified_at,f.rel_path,f.last_verified_at
                 FROM files f JOIN trash t ON t.file_id = f.id
                 ORDER BY t.trashed_at DESC LIMIT ?1",
            )
            .map_err(db_e)?;
        let items = st
            .query_map(params![limit as i64], |r| {
                let rel: String = r.get(8)?;
                Ok(FileObject {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    category: r.get(2)?,
                    mime: r.get(3)?,
                    size: r.get::<_, i64>(4)? as u64,
                    hash: r.get(5)?,
                    created_at: r.get(6)?,
                    modified_at: r.get(7)?,
                    path: rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default(),
                    last_verified_at: r.get(9)?,
                })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        Ok(Page { items, next_cursor: None })
    }

    pub fn restore_file(&self, id: &str) -> Result<()> {
        let mut c = self.db.lock()?;
        let tx = c.transaction().map_err(db_e)?;
        tx.execute("UPDATE files SET deleted_at=NULL WHERE id=?1", params![id])
            .map_err(db_e)?;
        tx.execute("DELETE FROM trash WHERE file_id=?1", params![id])
            .map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(())
    }

    /// Explicit purge (admin scope) — the only hard delete in the system,
    /// and only from trash (AGENTS.md §2.2).
    pub fn purge_file(&self, id: &str) -> Result<()> {
        let path = self.file_disk_path(id).ok();
        let mut c = self.db.lock()?;
        let tx = c.transaction().map_err(db_e)?;
        tx.execute("DELETE FROM files WHERE id=?1", params![id]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        drop(c);
        if let Some(p) = path {
            let _ = std::fs::remove_file(p);
        }
        Ok(())
    }

    /// Retention GC: purge trash entries past purge_after (BACKEND_SCHEMA §11).
    pub fn purge_expired_trash(&self) -> Result<u32> {
        let expired: Vec<String> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare("SELECT file_id FROM trash WHERE purge_after < ?1")
                .map_err(db_e)?;
            let rows = st
                .query_map(params![now_ms()], |r| r.get(0))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(db_e)?;
            rows
        };
        let mut n = 0;
        for id in expired {
            self.purge_file(&id)?;
            n += 1;
        }
        Ok(n)
    }
}
