//! Incremental verified external copies, independently retained after deletion.
use hh_core::{Error,Result};
use hh_core::time::now_ms;
use rusqlite::params;
use crate::{db_e,StorageService};

impl StorageService {
    pub fn run_second_copy(&self)->Result<(u64,u64,String)>{self.second_copy_with_job(None)}
    pub(crate) fn second_copy_with_job(&self,job_id:Option<&str>)->Result<(u64,u64,String)> {
        let target=self.db.get_setting("second_copy.root")?.filter(|s|!s.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone()).ok_or_else(||Error::BadRequest("choose a second-copy drive".into()))?;
        // Never create an absent drive's mount point and pretend it is attached.
        if !target.is_dir(){return Err(Error::StorageUnavailable("second-copy drive is disconnected".into()));}
        let target=target.canonicalize()?;
        let library=self.cfg.library_root.canonicalize()?;
        if target.starts_with(&library)||library.starts_with(&target){return Err(Error::BadRequest("second copy requires an independent location".into()));}
        // Stable marker survives drive-letter changes; never format/delete drive.
        let marker=target.join(".homehub-drive-id");
        let target_id=if marker.exists(){std::fs::read_to_string(&marker)?.trim().to_string()}else{
            let id=ulid::Ulid::new().to_string();use std::io::Write;let mut f=std::fs::OpenOptions::new().create_new(true).write(true).open(&marker)?;f.write_all(id.as_bytes())?;f.sync_all()?;id
        };
        if target_id.parse::<ulid::Ulid>().is_err(){return Err(Error::BadRequest("invalid second-copy drive marker".into()));}
        let run_id=ulid::Ulid::new().to_string();
        {let c=self.db.lock()?;c.execute("INSERT INTO storage_roots(id,kind,path,label,created_at) VALUES(?1,'second_copy',?2,'Second copy',?3) ON CONFLICT(id) DO UPDATE SET path=excluded.path,is_active=1",params![target_id,target.to_string_lossy(),now_ms()]).map_err(db_e)?;
            c.execute("INSERT INTO second_copy_runs(id,target_root_id,started_at,status) VALUES(?1,?2,?3,'running')",params![run_id,target_id,now_ms()]).map_err(db_e)?;}
        let files:Vec<(String,String,i64)>={let c=self.db.lock()?;let mut st=c.prepare("SELECT id,hash,size FROM files WHERE deleted_at IS NULL").map_err(db_e)?;let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
        let(mut copied,mut failed,mut bytes)=(0u64,0u64,0u64);
        let total=files.len();
        for (index,(id,hash,size)) in files.into_iter().enumerate() {
            if let Some(job)=job_id {if self.cancelled(job)?{let c=self.db.lock()?;c.execute("UPDATE second_copy_runs SET status='cancelled',finished_at=?2,files_copied=?3,bytes_copied=?4,files_failed=?5 WHERE id=?1",params![run_id,now_ms(),copied as i64,bytes as i64,failed as i64]).map_err(db_e)?;return Err(Error::Conflict("cancelled".into()));}self.job_progress(job,"running",total,index,None)?;}
            let rel=format!("HomeHubCopies/{id}/{hash}");let dst=hh_core::paths::jail_join(&target,&rel)?;
            let result=(||->Result<bool>{
                let src={let c=self.db.lock()?;crate::library::disk_path(&c,&id,false)?};
                let mut new=false;
                if dst.exists(){if hh_transfer::hash_file(&std::fs::File::open(&dst)?)?!=hash{crate::verified_replace(&src,&dst,&hash)?;new=true;}}
                else {crate::verified_copy(&src,&dst,&hash)?;new=true;}
                if dst.metadata()?.len()!=size as u64{return Err(Error::RootHashMismatch);}
                let c=self.db.lock()?;
                c.execute("INSERT INTO second_copy_files(file_id,target_root_id,rel_path,hash,size,verified_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(file_id,target_root_id) DO UPDATE SET rel_path=excluded.rel_path,hash=excluded.hash,size=excluded.size,verified_at=excluded.verified_at",params![id,target_id,rel,hash,size,now_ms()]).map_err(db_e)?;
                Ok(new)
            })();
            match result {Ok(true)=>{copied+=1;bytes+=size as u64;},Ok(false)=>{},Err(e)=>{
                failed+=1;
                let c=self.db.lock()?;
                c.execute("DELETE FROM second_copy_files WHERE file_id=?1 AND target_root_id=?2",params![id,target_id]).map_err(db_e)?;
                tracing::warn!(file_id=%id,error=%e,"second copy failed");
            }}
        }
        let c=self.db.lock()?;c.execute("UPDATE second_copy_runs SET finished_at=?2,files_copied=?3,bytes_copied=?4,files_failed=?5,status=?6 WHERE id=?1",params![run_id,now_ms(),copied as i64,bytes as i64,failed as i64,if failed==0{"ok"}else{"partial"}]).map_err(db_e)?;
        Ok((copied,failed,run_id))
    }
    pub fn last_second_copy_age_ms(&self)->Result<Option<i64>> {
        let c=self.db.lock()?;let last:Option<i64>=c.query_row("SELECT MAX(finished_at) FROM second_copy_runs WHERE status='ok'",[],|r|r.get(0)).map_err(db_e)?;Ok(last.map(|t|now_ms()-t))
    }
    pub fn copy_coverage(&self)->Result<serde_json::Value>{
        let configured=self.db.get_setting("second_copy.root")?.filter(|v|!v.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone());
        let connected=configured.as_deref().filter(|path|path.is_dir()).and_then(|path|std::fs::read_to_string(path.join(".homehub-drive-id")).ok()).map(|id|id.trim().to_owned()).filter(|id|id.parse::<ulid::Ulid>().is_ok());
        let c=self.db.lock()?;
        let total:i64=c.query_row("SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",[],|r|r.get(0)).map_err(db_e)?;
        let covered:i64=if let Some(id)=&connected {c.query_row("SELECT COUNT(*) FROM files f WHERE f.deleted_at IS NULL AND EXISTS(SELECT 1 FROM second_copy_files s JOIN storage_roots r ON r.id=s.target_root_id WHERE s.file_id=f.id AND s.hash=f.hash AND s.size=f.size AND r.is_active=1 AND r.id=?1)",params![id],|r|r.get(0)).map_err(db_e)?}else{0};
        let fresh:i64=if let Some(id)=&connected {c.query_row("SELECT COUNT(*) FROM files f WHERE f.deleted_at IS NULL AND EXISTS(SELECT 1 FROM second_copy_files s JOIN storage_roots r ON r.id=s.target_root_id WHERE s.file_id=f.id AND s.hash=f.hash AND s.size=f.size AND s.verified_at>=?2 AND r.is_active=1 AND r.id=?1)",params![id,now_ms()-24*60*60*1000],|r|r.get(0)).map_err(db_e)?}else{0};
        Ok(serde_json::json!({"total_files":total,"covered_files":covered,"drive_connected":connected.is_some(),"all_protected":total>0&&total==covered,"freshly_verified":total>0&&total==fresh}))
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn freshness_requires_each_live_file_to_be_recently_verified() {
        let root=std::env::temp_dir().join(format!("hh-coverage-test-{}",ulid::Ulid::new()));
        let library=root.join("library");let drive=root.join("drive");
        std::fs::create_dir_all(&library).unwrap();std::fs::create_dir_all(&drive).unwrap();
        let drive_id=ulid::Ulid::new().to_string();
        std::fs::write(drive.join(".homehub-drive-id"),&drive_id).unwrap();
        let db=hh_db::Db::open_memory().unwrap();
        {
            let c=db.lock().unwrap();
            c.execute("INSERT INTO storage_roots(id,kind,path,created_at) VALUES('lib','library',?1,0)",params![library.to_string_lossy()]).unwrap();
            c.execute("INSERT INTO storage_roots(id,kind,path,created_at) VALUES(?1,'second_copy',?2,0)",params![drive_id,drive.to_string_lossy()]).unwrap();
            c.execute("INSERT INTO files(id,root_id,rel_path,name,category,size,hash,created_at) VALUES('file','lib','a','a','document',3,'hash',0)",[]).unwrap();
            c.execute("INSERT INTO second_copy_files(file_id,target_root_id,rel_path,hash,size,verified_at) VALUES('file',?1,'copy','hash',3,?2)",params![drive_id,now_ms()-2*24*60*60*1000]).unwrap();
        }
        let mut cfg=hh_core::Config::default();cfg.library_root=library;cfg.second_copy_root=Some(drive);
        let service=StorageService::new(db.clone(),cfg);
        let old=service.copy_coverage().unwrap();
        assert_eq!(old["all_protected"],true);assert_eq!(old["freshly_verified"],false);
        {
            let c=db.lock().unwrap();
            c.execute("UPDATE second_copy_files SET verified_at=?1",params![now_ms()]).unwrap();
        }
        assert_eq!(service.copy_coverage().unwrap()["freshly_verified"],true);
        {
            let c=db.lock().unwrap();
            c.execute("DELETE FROM second_copy_files",[]).unwrap();
        }
        assert_eq!(service.copy_coverage().unwrap()["all_protected"],false);
        std::fs::remove_dir_all(root).unwrap();
    }
}
