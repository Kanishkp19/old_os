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
        let groups: Vec<String> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare(
                    "SELECT hash FROM files
                     WHERE deleted_at IS NULL GROUP BY hash HAVING COUNT(*) > 1",
                )
                .map_err(db_e)?;
            let rows = st
                .query_map([], |r| r.get(0))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(db_e)?;
            rows
        };

        let mut created = 0;
        for hash in groups {
            let candidates: Vec<(String,u64)> = {
                let c = self.db.lock()?;
                let mut st = c
                    .prepare("SELECT id,size FROM files WHERE hash=?1 AND deleted_at IS NULL ORDER BY created_at")
                    .map_err(db_e)?;
                let rows = st
                    .query_map(params![hash], |r| Ok((r.get(0)?,r.get::<_,i64>(1)? as u64)))
                    .map_err(db_e)?
                    .collect::<std::result::Result<Vec<(String,u64)>, _>>()
                    .map_err(db_e)?;
                rows
            };
            let mut file_ids = Vec::new();
            let mut size = 0;
            for (id, expected_size) in candidates {
                let path = {let c=self.db.lock()?;crate::library::disk_path(&c,&id,false)?};
                let file = std::fs::File::open(path)?;
                if file.metadata()?.len() == expected_size && hh_transfer::hash_file(&file)?.eq_ignore_ascii_case(&hash) {
                    size=expected_size;file_ids.push(id);
                }
            }
            if file_ids.len() < 2 { continue; }
            let reclaimable = size.saturating_mul(file_ids.len() as u64 - 1);
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
            if let Some(existing) = &exists {
                let mut st=tx.prepare("SELECT file_id FROM duplicate_members WHERE group_id=?1 ORDER BY file_id").map_err(db_e)?;
                let mut old=st.query_map(params![existing],|r|r.get::<_,String>(0)).map_err(db_e)?
                    .collect::<std::result::Result<Vec<_>,_>>().map_err(db_e)?;
                drop(st);
                old.sort();let mut fresh=file_ids.clone();fresh.sort();
                if old != fresh {tx.execute("UPDATE duplicate_groups SET status='dismissed' WHERE id=?1",params![existing]).map_err(db_e)?;}
                else {tx.commit().map_err(db_e)?;continue;}
            }
            {
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
        let (hash, members): (String, Vec<(String,u64)>) = {
            let c = self.db.lock()?;
            let hash:String=c.query_row("SELECT hash FROM duplicate_groups WHERE id=?1 AND kind='exact' AND status='open'",params![group_id],|r|r.get(0))
                .map_err(|_|hh_core::Error::NotFound(format!("open duplicate group {group_id}")))?;
            let mut st = c
                .prepare("SELECT f.id,f.size FROM duplicate_members m JOIN files f ON f.id=m.file_id WHERE m.group_id=?1 AND f.deleted_at IS NULL AND f.hash=?2")
                .map_err(db_e)?;
            let rows = st
                .query_map(params![group_id,hash], |r| Ok((r.get(0)?,r.get::<_,i64>(1)? as u64)))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<(String,u64)>, _>>()
                .map_err(db_e)?;
            (hash,rows)
        };
        if members.len()<2 || !members.iter().any(|(id,_)|id==keep_file_id) { return Err(hh_core::Error::BadRequest("keeper is not an active group member".into())); }
        // Preflight every member before any destructive action.
        for (fid,size) in &members {
            let path={let c=self.db.lock()?;crate::library::disk_path(&c,fid,false)?};
            let file=std::fs::File::open(path)?;
            if file.metadata()?.len()!=*size || !hh_transfer::hash_file(&file)?.eq_ignore_ascii_case(&hash) {
                return Err(hh_core::Error::Conflict("duplicate group changed; scan again".into()));
            }
        }
        let mut c=self.db.lock()?;let tx=c.transaction().map_err(db_e)?;
        for (fid,_) in &members {
            if fid==keep_file_id {continue;}
            crate::library::assert_mutable(&tx,fid)?;
        }
        let when=now_ms();
        for (fid,_) in &members {
            if fid==keep_file_id {continue;}
            if tx.execute("UPDATE files SET deleted_at=?2 WHERE id=?1 AND deleted_at IS NULL AND hash=?3",params![fid,when,hash]).map_err(db_e)?!=1 {
                return Err(hh_core::Error::Conflict("duplicate group changed; scan again".into()));
            }
            tx.execute("INSERT OR REPLACE INTO trash(file_id,trashed_at,trashed_by_device_id,purge_after) VALUES(?1,?2,NULL,?3)",params![fid,when,when+hh_core::TRASH_RETENTION_MS]).map_err(db_e)?;
            tx.execute("INSERT INTO audit_log(ts,action,detail) VALUES(?1,'file_delete',?2)",params![when,fid]).map_err(db_e)?;
        }
        tx.execute("UPDATE duplicate_groups SET status='resolved' WHERE id=?1",params![group_id]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(())
    }
}
