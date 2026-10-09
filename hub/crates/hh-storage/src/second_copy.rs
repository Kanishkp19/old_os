//! Incremental verified external copies, independently retained after deletion.
use hh_core::{Error,Result};
use hh_core::time::now_ms;
use rusqlite::{params,OptionalExtension};
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
        if same_volume(&target,&library)? {return Err(Error::BadRequest("second copy requires a different volume".into()));}
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
            if std::fs::read_to_string(&marker).map(|value|value.trim()!=target_id).unwrap_or(true) {
                let c=self.db.lock()?;
                c.execute("UPDATE second_copy_runs SET status='partial',finished_at=?2,files_copied=?3,bytes_copied=?4,files_failed=?5 WHERE id=?1",params![run_id,now_ms(),copied as i64,bytes as i64,failed as i64]).map_err(db_e)?;
                return Err(Error::StorageUnavailable("second-copy drive disconnected or changed".into()));
            }
            if let Some(job)=job_id {if self.cancelled(job)?{let c=self.db.lock()?;c.execute("UPDATE second_copy_runs SET status='partial',finished_at=?2,files_copied=?3,bytes_copied=?4,files_failed=?5 WHERE id=?1",params![run_id,now_ms(),copied as i64,bytes as i64,failed as i64]).map_err(db_e)?;return Err(Error::Conflict("cancelled".into()));}self.job_progress(job,"running",total,index,None)?;}
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
        let configured=self.db.get_setting("second_copy.root")?.filter(|v|!v.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone());
        let target_id=configured.as_deref().filter(|path|path.is_dir()).and_then(|path|std::fs::read_to_string(path.join(".homehub-drive-id")).ok()).map(|id|id.trim().to_owned()).filter(|id|id.parse::<ulid::Ulid>().is_ok());
        let Some(target_id)=target_id else {return Ok(None)};
        let c=self.db.lock()?;let last:Option<i64>=c.query_row("SELECT MAX(finished_at) FROM second_copy_runs WHERE status='ok' AND target_root_id=?1",[target_id],|r|r.get(0)).map_err(db_e)?;Ok(last.map(|t|now_ms().saturating_sub(t)))
    }
    pub fn copy_coverage(&self)->Result<serde_json::Value>{
        let configured=self.db.get_setting("second_copy.root")?.filter(|v|!v.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone());
        let connected=configured.as_deref().filter(|path|path.is_dir()).and_then(|path|std::fs::read_to_string(path.join(".homehub-drive-id")).ok()).map(|id|id.trim().to_owned()).filter(|id|id.parse::<ulid::Ulid>().is_ok());
        let c=self.db.lock()?;
        let files:Vec<(String,String,u64)>=c.prepare("SELECT id,hash,size FROM files WHERE deleted_at IS NULL").map_err(db_e)?
            .query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)? as u64))).map_err(db_e)?
            .collect::<std::result::Result<_,_>>().map_err(db_e)?;
        let latest_status:Option<String>=if let Some(id)=&connected {c.query_row("SELECT status FROM second_copy_runs WHERE target_root_id=?1 ORDER BY started_at DESC LIMIT 1",params![id],|r|r.get(0)).optional().map_err(db_e)?}else{None};
        drop(c);
        let mut covered=0usize;let mut fresh=0usize;
        if let (Some(root),Some(id))=(configured.as_deref(),connected.as_deref()) {
            for (file_id,hash,size) in &files {
                if let Some(verified_at)=self.verified_copy_at(root,id,file_id,hash,*size)? {
                    covered+=1;
                    if verified_at>=now_ms()-24*60*60*1000 {fresh+=1;}
                }
            }
        }
        let complete=latest_status.as_deref()==Some("ok") && !files.is_empty() && files.len()==covered;
        Ok(serde_json::json!({"total_files":files.len(),"covered_files":covered,"drive_connected":connected.is_some(),"all_protected":complete,"freshly_verified":complete&&files.len()==fresh}))
    }

    pub fn file_second_copy_status(&self,file_id:&str,hash:&str,size:u64)->Result<String>{
        let configured=self.db.get_setting("second_copy.root")?.filter(|v|!v.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone());
        let Some(root)=configured else {return Ok("needs_copy".into())};
        let id=std::fs::read_to_string(root.join(".homehub-drive-id")).ok().map(|v|v.trim().to_owned()).filter(|v|v.parse::<ulid::Ulid>().is_ok());
        let Some(id)=id else {return Ok("disconnected".into())};
        Ok(if self.verified_copy_at(&root,&id,file_id,hash,size)?.is_some(){"verified"}else{"needs_copy"}.into())
    }

    fn verified_copy_at(&self,root:&std::path::Path,target_id:&str,file_id:&str,hash:&str,size:u64)->Result<Option<i64>>{
        let row:Option<(String,i64)>={let c=self.db.lock()?;
            c.query_row("SELECT s.rel_path,s.verified_at FROM second_copy_files s JOIN storage_roots r ON r.id=s.target_root_id WHERE s.file_id=?1 AND s.target_root_id=?2 AND s.hash=?3 AND s.size=?4 AND r.is_active=1",params![file_id,target_id,hash,size as i64],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_e)?};
        let Some((rel,verified_at))=row else {return Ok(None)};
        let path=match hh_core::paths::jail_join(root,&rel){Ok(path)=>path,Err(_)=>return Ok(None)};
        Ok(path.metadata().ok().filter(|m|m.is_file()&&m.len()==size).map(|_|verified_at))
    }
}

fn same_volume(a:&std::path::Path,b:&std::path::Path)->Result<bool> {
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;return Ok(std::fs::metadata(a)?.dev()==std::fs::metadata(b)?.dev());}
    #[cfg(windows)] {return Ok(a.components().next()==b.components().next());}
    #[cfg(not(any(unix,windows)))] {let _=(a,b);Ok(false)}
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
