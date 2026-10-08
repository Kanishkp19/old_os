//! Incremental verified external copies, independently retained after deletion.
use hh_core::{Error,Result};
use hh_core::time::now_ms;
use rusqlite::params;
use crate::{db_e,StorageService};

impl StorageService {
    pub fn run_second_copy(&self)->Result<(u64,u64,String)> {
        let target=self.db.get_setting("second_copy.root")?.filter(|s|!s.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone()).ok_or_else(||Error::BadRequest("choose a second-copy drive".into()))?;
        // Never create an absent drive's mount point and pretend it is attached.
        if !target.is_dir(){return Err(Error::StorageUnavailable("second-copy drive is disconnected".into()));}
        let target=target.canonicalize()?;
        if target.starts_with(self.cfg.library_root.canonicalize()?){return Err(Error::BadRequest("second copy requires an independent location".into()));}
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
        for (id,hash,size) in files {
            let rel=format!("HomeHubCopies/{id}/{hash}");let dst=hh_core::paths::jail_join(&target,&rel)?;
            let result=(||->Result<bool>{
                let c=self.db.lock()?;let src=crate::library::disk_path(&c,&id,false)?;
                let mut new=false;
                if dst.exists(){if hh_transfer::hash_file(&std::fs::File::open(&dst)?)?!=hash{return Err(Error::RootHashMismatch);}}
                else {crate::verified_copy(&src,&dst,&hash)?;new=true;}
                if dst.metadata()?.len()!=size as u64{return Err(Error::RootHashMismatch);}
                c.execute("INSERT INTO second_copy_files(file_id,target_root_id,rel_path,hash,size,verified_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(file_id,target_root_id) DO UPDATE SET rel_path=excluded.rel_path,hash=excluded.hash,size=excluded.size,verified_at=excluded.verified_at",params![id,target_id,rel,hash,size,now_ms()]).map_err(db_e)?;
                Ok(new)
            })();
            match result {Ok(true)=>{copied+=1;bytes+=size as u64;},Ok(false)=>{},Err(e)=>{failed+=1;tracing::warn!(file_id=%id,error=%e,"second copy failed");}}
        }
        let c=self.db.lock()?;c.execute("UPDATE second_copy_runs SET finished_at=?2,files_copied=?3,bytes_copied=?4,files_failed=?5,status=?6 WHERE id=?1",params![run_id,now_ms(),copied as i64,bytes as i64,failed as i64,if failed==0{"ok"}else{"partial"}]).map_err(db_e)?;
        Ok((copied,failed,run_id))
    }
    pub fn last_second_copy_age_ms(&self)->Result<Option<i64>> {
        let c=self.db.lock()?;let last:Option<i64>=c.query_row("SELECT MAX(finished_at) FROM second_copy_runs WHERE status='ok'",[],|r|r.get(0)).map_err(db_e)?;Ok(last.map(|t|now_ms()-t))
    }
    pub fn copy_coverage(&self)->Result<serde_json::Value>{
        let c=self.db.lock()?;
        let total:i64=c.query_row("SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",[],|r|r.get(0)).map_err(db_e)?;
        let covered:i64=c.query_row("SELECT COUNT(*) FROM files f WHERE f.deleted_at IS NULL AND EXISTS(SELECT 1 FROM second_copy_files s JOIN storage_roots r ON r.id=s.target_root_id WHERE s.file_id=f.id AND s.hash=f.hash AND s.size=f.size AND r.is_active=1)",[],|r|r.get(0)).map_err(db_e)?;
        Ok(serde_json::json!({"total_files":total,"covered_files":covered,"all_protected":total>0&&total==covered,"freshly_verified":false}))
    }
}
