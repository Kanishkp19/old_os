//! Exact duplicate detection (TRD §7.3, FR-5.5): size match → full BLAKE3
//! hash → groups. Never auto-deletes; resolution moves losers to trash.

use hh_core::error::Result;
use hh_core::time::now_ms;
use rusqlite::params;
use serde::Serialize;

use crate::{db_e, StorageService};

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub kind: String,
    pub hash: Option<String>,
    pub reclaimable_bytes: u64,
    pub file_ids: Vec<String>,
}

impl StorageService {
    /// Scan for exact duplicates and (re)build duplicate_groups.
    pub fn scan_duplicates(&self) -> Result<u32> {
        // Groups of non-deleted files sharing hash with count > 1.
        let groups: Vec<(String, i64, i64)> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare(
                    "SELECT hash, COUNT(*), SUM(size) FROM files
                     WHERE deleted_at IS NULL GROUP BY hash HAVING COUNT(*) > 1",
                )
                .map_err(db_e)?;
            let rows = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<(String, i64, i64)>, _>>()
                .map_err(db_e)?;
            rows
        };

        let mut created = 0;
        for (hash, count, total_bytes) in groups {
            let reclaimable = total_bytes - (total_bytes / count); // keep one copy
            let file_ids: Vec<String> = {
                let c = self.db.lock()?;
                let mut st = c
                    .prepare("SELECT id FROM files WHERE hash=?1 AND deleted_at IS NULL ORDER BY created_at")
                    .map_err(db_e)?;
                let rows = st
                    .query_map(params![hash], |r| r.get(0))
                    .map_err(db_e)?
                    .collect::<std::result::Result<Vec<String>, _>>()
                    .map_err(db_e)?;
                rows
            };
            let mut c = self.db.lock()?;
            let tx = c.transaction().map_err(db_e)?;
            // Skip if an open group already exists for this hash.
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM duplicate_groups WHERE hash=?1 AND status='open'",
                    params![hash],
                    |r| r.get(0),
                )
                .ok();
            if exists.is_none() {
                let gid = ulid::Ulid::new().to_string();
                tx.execute(
                    "INSERT INTO duplicate_groups (id, kind, hash, reclaimable_bytes, detected_at)
                     VALUES (?1,'exact',?2,?3,?4)",
                    params![gid, hash, reclaimable, now_ms()],
                )
                .map_err(db_e)?;
                for (i, fid) in file_ids.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO duplicate_members (group_id, file_id, is_keeper) VALUES (?1,?2,?3)",
                        params![gid, fid, (i == 0) as i64],
                    )
                    .map_err(db_e)?;
                }
                created += 1;
            }
            tx.commit().map_err(db_e)?;
        }
        Ok(created)
    }

    pub fn list_duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT id, kind, hash, reclaimable_bytes FROM duplicate_groups WHERE status='open'")
            .map_err(db_e)?;
        let mut groups: Vec<DuplicateGroup> = st
            .query_map([], |r| {
                Ok(DuplicateGroup {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    hash: r.get(2)?,
                    reclaimable_bytes: r.get::<_, i64>(3)? as u64,
                    file_ids: vec![],
                })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        for g in &mut groups {
            let mut st = c
                .prepare("SELECT file_id FROM duplicate_members WHERE group_id=?1")
                .map_err(db_e)?;
            g.file_ids = st
                .query_map(params![g.id], |r| r.get(0))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(db_e)?;
        }
        Ok(groups)
    }

    /// Keep `keep_file_id`, move all other members to trash (TRD §7.3).
    pub fn resolve_duplicate_group(&self, group_id: &str, keep_file_id: &str) -> Result<()> {
        let members: Vec<String> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare("SELECT file_id FROM duplicate_members WHERE group_id=?1")
                .map_err(db_e)?;
            let rows = st
                .query_map(params![group_id], |r| r.get(0))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(db_e)?;
            rows
        };
        if !members.iter().any(|id|id==keep_file_id) { return Err(hh_core::Error::BadRequest("keeper is not a group member".into())); }
        // Preflight every member before any destructive action.
        { let c=self.db.lock()?; for fid in &members { if fid!=keep_file_id {crate::library::assert_mutable(&c,fid)?;} } }
        for fid in members {
            if fid != keep_file_id {
                self.trash_file(&fid, None)?;
            }
        }
        let c = self.db.lock()?;
        c.execute(
            "UPDATE duplicate_groups SET status='resolved' WHERE id=?1",
            params![group_id],
        )
        .map_err(db_e)?;
        Ok(())
    }
}
