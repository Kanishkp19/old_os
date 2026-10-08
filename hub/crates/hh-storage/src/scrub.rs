//! Integrity scrubber (TRD §7.4, FR-6.2): rolling re-hash of stored files
//! at idle priority, `integrity_events` on mismatch, repair-from-second-copy
//! hook (ST-06).

use hh_core::error::Result;
use hh_core::time::now_ms;
use rusqlite::params;

use crate::{db_e, StorageService};

impl StorageService {
    /// Scrub up to `max_files` least-recently-verified files.
    /// Returns (checked, mismatched).
    pub fn scrub_once(&self,max_files:u32)->Result<(u32,u32)>{self.scrub_with_job(max_files,None)}
    pub(crate) fn scrub_with_job(&self,max_files:u32,job_id:Option<&str>)->Result<(u32,u32)> {
        let candidates: Vec<(String, String, String)> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare(
                    "SELECT id, rel_path, hash FROM files
                     WHERE deleted_at IS NULL
                     ORDER BY COALESCE(last_verified_at, 0) ASC LIMIT ?1",
                )
                .map_err(db_e)?;
            let rows = st
                .query_map(params![max_files as i64], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                })
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_e)?;
            rows
        };

        let mut checked = 0;
        let mut mismatched = 0;
        let total=candidates.len();
        for (index,(id, rel, expected)) in candidates.into_iter().enumerate() {
            if let Some(job)=job_id {if self.cancelled(job)?{return Err(hh_core::Error::Conflict("cancelled".into()));}self.job_progress(job,"running",total,index,None)?;}
            let c=self.db.lock()?;
            let path=crate::library::disk_path(&c,&id,false)?;
            drop(c);
            let outcome = match std::fs::File::open(&path) {
                Ok(f) => hh_transfer::hash_file(&f),
                Err(_) => {
                    self.record_integrity_event(Some(&id), "missing", Some(&rel))?;
                    mismatched += 1;
                    continue;
                }
            };
            match outcome {
                Ok(actual) if actual.eq_ignore_ascii_case(&expected) => {
                    {
                        let c = self.db.lock()?;
                        c.execute(
                            "UPDATE files SET last_verified_at=?2 WHERE id=?1",
                            params![id, now_ms()],
                        )
                        .map_err(db_e)?;
                    }
                    self.record_integrity_event(Some(&id), "scrub_ok", None)?;
                    checked += 1;
                }
                Ok(actual) => {
                    mismatched += 1;
                    self.record_integrity_event(
                        Some(&id),
                        "hash_mismatch",
                        Some(&format!("expected {expected}, got {actual}")),
                    )?;
                    self.db.create_alert(
                        "critical",
                        "INTEGRITY_MISMATCH",
                        "A stored file failed its integrity check. If you have a second copy, we'll repair from it.",
                    )?;
                    // A cleanup lease may still have an Android system dialog
                    // open. Preserve its copy and expose the integrity failure.
                    let c=self.db.lock()?;
                    if crate::library::assert_mutable(&c,&id).is_ok() {
                        let copies:Vec<(String,String)>={let mut st=c.prepare("SELECT r.path,s.rel_path FROM second_copy_files s JOIN storage_roots r ON r.id=s.target_root_id WHERE s.file_id=?1 AND s.hash=?2 AND r.is_active=1").map_err(db_e)?;let rows=st.query_map(params![id,expected],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
                        for (root,rel) in copies {
                            let repair=(||->Result<()> {
                                let src=hh_core::paths::jail_join(std::path::Path::new(&root),&rel)?;
                                let temp=path.with_file_name(format!(".hh-repair-{}",ulid::Ulid::new()));
                                crate::verified_copy(&src,&temp,&expected)?;
                                crate::atomic_replace(&temp,&path)?;
                                c.execute("UPDATE files SET last_verified_at=?2 WHERE id=?1",params![id,now_ms()]).map_err(db_e)?;
                                Ok(())
                            })();
                            match repair {Ok(())=>{c.execute("INSERT INTO integrity_events(id,file_id,kind,detected_at) VALUES(?1,?2,'repaired_from_copy',?3)",params![ulid::Ulid::new().to_string(),id,now_ms()]).map_err(db_e)?;break;},Err(e)=>tracing::warn!(file_id=%id,error=%e,"integrity repair did not succeed")}
                        }
                    }
                }
                Err(e) => {
                    mismatched += 1;
                    self.record_integrity_event(Some(&id), "read_error", Some(&e.to_string()))?;
                }
            }
        }
        Ok((checked, mismatched))
    }

    fn record_integrity_event(&self, file_id: Option<&str>, kind: &str, detail: Option<&str>) -> Result<()> {
        let c = self.db.lock()?;
        c.execute(
            "INSERT INTO integrity_events (id, file_id, kind, detected_at, detail)
             VALUES (?1,?2,?3,?4,?5)",
            params![ulid::Ulid::new().to_string(), file_id, kind, now_ms(), detail],
        )
        .map_err(db_e)?;
        Ok(())
    }
}
