//! Durable, cancellable copy/import jobs. Originals are always preserved.
use std::path::{Path,PathBuf};
use hh_core::{Error,Result};
use hh_core::time::now_ms;
use rusqlite::params;
use serde_json::{json,Value};
use crate::{StorageService,db_e};

impl StorageService {
    pub fn scan_import(&self,paths:&[String])->Result<Value> {
        let files=discover(paths)?;
        let mut bytes=0u64;
        let mut categories:std::collections::BTreeMap<String,(u64,u64)>=std::collections::BTreeMap::new();
        for p in &files {
            let size=p.metadata()?.len();bytes=bytes.saturating_add(size);
            let name=p.file_name().map(|v|v.to_string_lossy()).unwrap_or_default();
            let category=hh_core::paths::category_for_mime(Some(mime_for(&name))).to_string();
            let entry=categories.entry(category).or_default();entry.0+=1;entry.1=entry.1.saturating_add(size);
        }
        let categories:std::collections::BTreeMap<_,_>=categories.into_iter().map(|(name,(files,bytes))|(name,json!({"files":files,"bytes":bytes}))).collect();
        Ok(json!({"files":files.len(),"bytes":bytes,"categories":categories,"default_choice":"later","originals_preserved":true}))
    }

    pub fn start_import(&self,paths:Vec<String>,mode:&str)->Result<Value> {
        if mode=="later" {return Ok(json!({"choice":"later","job":null}));}
        if mode!="copy" && mode!="keep" {return Err(Error::BadRequest("choose copy, keep or later".into()));}
        let id=self.create_job("import",json!({"paths":paths,"mode":mode}))?;
        let service=self.clone();let job_id=id.clone();let mode=mode.to_string();
        std::thread::spawn(move||{
            let result=(||->Result<Value>{
                let payload=service.job(&job_id)?;let paths:Vec<String>=serde_json::from_value(payload["payload"]["paths"].clone()).map_err(|e|Error::BadRequest(e.to_string()))?;
                let files=discover(&paths)?;service.job_progress(&job_id,"running",files.len(),0,None)?;
                let mut ids=Vec::new();
                for (n,path) in files.iter().enumerate(){
                    if service.cancelled(&job_id)? {return Err(Error::Conflict("cancelled".into()));}
                    ids.push(service.import_one(path,&mode,None,None)?);
                    service.job_progress(&job_id,"running",files.len(),n+1,None)?;
                }
                Ok(json!({"file_ids":ids,"originals_preserved":true}))
            })();service.finish_job(&job_id,result);
        });
        self.job(&id)
    }

    pub(crate) fn import_one(&self,path:&Path,mode:&str,rel:Option<&str>,expected:Option<&str>)->Result<String> {
        let src=path.canonicalize()?;
        if !src.is_file(){return Err(Error::BadRequest("import requires regular file".into()));}
        let input=std::fs::File::open(&src)?;let meta=input.metadata()?;
        let hash=hh_transfer::hash_file(&input)?;
        if expected.is_some_and(|h|!h.eq_ignore_ascii_case(&hash)){return Err(Error::RootHashMismatch);}
        let name=hh_core::paths::sanitize_component(&src.file_name().ok_or_else(||Error::BadRequest("filename".into()))?.to_string_lossy())?;
        let mime=mime_for(&name);let category=hh_core::paths::category_for_mime(Some(mime));
        let id=ulid::Ulid::new().to_string();
        let keep=mode=="keep";
        let root=if keep {src.parent().ok_or_else(||Error::BadRequest("parent".into()))?.to_path_buf()}else{self.cfg.library_dir()};
        std::fs::create_dir_all(&root)?;
        let mut c=self.db.lock()?;
        let root_text=root.to_string_lossy().to_string();
        let root_id:String=c.query_row("SELECT id FROM storage_roots WHERE path=?1 AND kind=?2 AND is_active=1",params![root_text,if keep{"import_source"}else{"library"}],|r|r.get(0)).unwrap_or_else(|_|ulid::Ulid::new().to_string());
        c.execute("INSERT OR IGNORE INTO storage_roots(id,kind,path,created_at) VALUES(?1,?2,?3,?4)",params![root_id,if keep{"import_source"}else{"library"},root_text,now_ms()]).map_err(db_e)?;
        let rel=if keep {src.file_name().ok_or_else(||Error::BadRequest("filename".into()))?.to_string_lossy().to_string()}else if let Some(rel)=rel{hh_core::paths::sanitize_rel_path(rel)?}else{format!("Imports/{id}/{name}")};
        let dst=hh_core::paths::jail_join(&root,&rel)?;
        if keep {
            let existing:Option<String>=c.query_row("SELECT id FROM files WHERE root_id=?1 AND rel_path=?2",params![root_id,rel],|r|r.get(0)).ok();
            if let Some(id)=existing {return Ok(id);}
        } else if fs2::free_space(&root)?<meta.len().saturating_add(16*1024*1024) {return Err(Error::StorageFull);}
        let stored_name=dst.file_name().ok_or_else(||Error::BadRequest("filename".into()))?.to_string_lossy().to_string();
        let payload=json!({"root_id":root_id,"rel_path":rel,"name":stored_name,"category":category,"mime":mime,"size":meta.len(),"hash":hash,"source_mode":if keep{"keep_in_place"}else{"import"}});
        c.execute("INSERT INTO file_operation_journal(id,file_id,kind,old_path,new_path,payload,state,created_at) VALUES(?1,?1,'import',?2,?3,?4,'prepared',?5)",params![id,src.to_string_lossy(),dst.to_string_lossy(),payload.to_string(),now_ms()]).map_err(db_e)?;
        if !keep {crate::verified_copy(&src,&dst,&hash)?;}
        let current=std::fs::File::open(if keep{&src}else{&dst})?;
        if current.metadata()?.len()!=meta.len() || hh_transfer::hash_file(&current)?!=hash {return Err(Error::RootHashMismatch);}
        let tx=c.transaction().map_err(db_e)?;insert_import(&tx,&id,&payload)?;
        insert_chunks(&tx,&id,&current)?;
        tx.execute("UPDATE file_operation_journal SET state='done' WHERE id=?1",params![id]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(id)
    }

    pub fn list_jobs(&self)->Result<Value>{
        let ids:Vec<String>={let c=self.db.lock()?;let mut st=c.prepare("SELECT id FROM jobs ORDER BY created_at DESC LIMIT 100").map_err(db_e)?;let rows=st.query_map([],|r|r.get(0)).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
        Ok(json!({"items":ids.iter().map(|id|self.job(id)).collect::<Result<Vec<_>>>()?}))
    }
    pub fn job(&self,id:&str)->Result<Value>{
        let c=self.db.lock()?;
        c.query_row("SELECT id,kind,status,total,done,payload,result,error,cancel_requested,created_at,updated_at FROM jobs WHERE id=?1",params![id],|r|{
            let payload:String=r.get(5)?;let result:Option<String>=r.get(6)?;
            Ok(json!({"id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"total":r.get::<_,i64>(3)?,"done":r.get::<_,i64>(4)?,"payload":serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null),"result":result.and_then(|s|serde_json::from_str::<Value>(&s).ok()),"error":r.get::<_,Option<String>>(7)?,"cancel_requested":r.get::<_,i64>(8)?!=0,"created_at":r.get::<_,i64>(9)?,"updated_at":r.get::<_,i64>(10)?}))
        }).map_err(|_|Error::NotFound("job".into()))
    }
    pub fn cancel_job(&self,id:&str)->Result<()> {
        let c=self.db.lock()?;let n=c.execute("UPDATE jobs SET cancel_requested=1,updated_at=?2 WHERE id=?1 AND status IN ('queued','running')",params![id,now_ms()]).map_err(db_e)?;
        if n==0{return Err(Error::Conflict("job cannot be cancelled".into()));}Ok(())
    }
    pub(crate) fn cancelled(&self,id:&str)->Result<bool>{let c=self.db.lock()?;c.query_row("SELECT cancel_requested FROM jobs WHERE id=?1",params![id],|r|Ok(r.get::<_,i64>(0)?!=0)).map_err(db_e)}
    fn create_job(&self,kind:&str,payload:Value)->Result<String>{let id=ulid::Ulid::new().to_string();let c=self.db.lock()?;
        let busy:i64=c.query_row("SELECT COUNT(*) FROM jobs WHERE status IN ('queued','running') AND (kind='library_move' OR ?1='library_move' OR kind=?1)",params![kind],|r|r.get(0)).map_err(db_e)?;
        if busy>0{return Err(Error::Conflict("another storage job must finish first".into()));}c.execute("INSERT INTO jobs(id,kind,status,payload,created_at,updated_at) VALUES(?1,?2,'queued',?3,?4,?4)",params![id,kind,payload.to_string(),now_ms()]).map_err(db_e)?;Ok(id)}
    pub(crate) fn job_progress(&self,id:&str,status:&str,total:usize,done:usize,error:Option<&str>)->Result<()> {let c=self.db.lock()?;c.execute("UPDATE jobs SET status=?2,total=?3,done=?4,error=?5,updated_at=?6 WHERE id=?1",params![id,status,total as i64,done as i64,error,now_ms()]).map_err(db_e)?;Ok(())}
    fn finish_job(&self,id:&str,result:Result<Value>){let update=(||->Result<()>{let c=self.db.lock()?;let (status,value,error)=match result{Ok(v)=>("completed",Some(v.to_string()),None),Err(e)=>(if c.query_row("SELECT cancel_requested FROM jobs WHERE id=?1",params![id],|r|r.get::<_,i64>(0)).map_err(db_e)?!=0{"cancelled"}else{"failed"},None,Some(e.to_string()))};c.execute("UPDATE jobs SET status=?2,result=?3,error=?4,updated_at=?5 WHERE id=?1",params![id,status,value,error,now_ms()]).map_err(db_e)?;Ok(())})();if let Err(e)=update{tracing::error!(error=%e,"could not persist job outcome");}}

    pub fn start_maintenance(&self,kind:&str)->Result<Value> {
        if !["scrub","second_copy","duplicates"].contains(&kind){return Err(Error::BadRequest("maintenance kind".into()));}
        let id=self.create_job(kind,json!({}))?;let service=self.clone();let job_id=id.clone();let kind=kind.to_string();
        std::thread::spawn(move||{let result=(||->Result<Value>{
            service.job_progress(&job_id,"running",0,0,None)?;
            match kind.as_str(){
                "scrub"=>{let(checked,failed)=service.scrub_with_job(1_000_000,Some(&job_id))?;Ok(json!({"checked":checked,"failed":failed}))},
                "second_copy"=>{let(copied,failed,run_id)=service.second_copy_with_job(Some(&job_id))?;Ok(json!({"copied":copied,"failed":failed,"run_id":run_id}))},
                _=>{if service.cancelled(&job_id)?{return Err(Error::Conflict("cancelled".into()));}let exact=service.scan_duplicates()?;let similar=service.scan_similar()?;Ok(json!({"exact_groups":exact,"similar_groups":similar,"automatic_deletion":false}))}
            }
        })();service.finish_job(&job_id,result);});self.job(&id)
    }

    /// Copies the library, verifies every destination, then switches the root.
    /// Old data stays intact. Sharing stays paused until the service restarts
    /// with the new persisted config, avoiding stale transfer-engine paths.
    pub fn start_library_move(&self,path:&str)->Result<Value>{
        let new=PathBuf::from(path);if !new.is_absolute(){return Err(Error::BadRequest("choose absolute library folder".into()));}
        {let c=self.db.lock()?;
            let active:i64=c.query_row("SELECT COUNT(*) FROM transfers WHERE status IN ('open','verifying')",[],|r|r.get(0)).map_err(db_e)?;
            let pins:i64=c.query_row("SELECT COUNT(*) FROM cleanup_leases WHERE state='active'",[],|r|r.get(0)).map_err(db_e)?;
            if active>0 || pins>0{return Err(Error::Conflict("finish transfers and pending phone cleanup before moving".into()));}
        }
        std::fs::create_dir_all(&new)?;let new=new.canonicalize()?;
        if new.starts_with(self.cfg.library_root.canonicalize()?) {return Err(Error::BadRequest("destination must be outside existing library".into()));}
        let id=self.create_job("library_move",json!({"path":new}))?;let service=self.clone();let job_id=id.clone();
        std::thread::spawn(move||{
            let result=(||->Result<Value>{
                let files:Vec<(String,String,String,i64)>={let c=service.db.lock()?;let mut st=c.prepare("SELECT f.id,f.rel_path,f.hash,f.size FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE r.kind='library'").map_err(db_e)?;let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
                let required=files.iter().map(|f|f.3.max(0) as u64).sum::<u64>();if fs2::free_space(&new)?<required.saturating_add(64*1024*1024){return Err(Error::StorageFull);}
                let dest=new.join("Library");std::fs::create_dir_all(&dest)?;
                service.job_progress(&job_id,"running",files.len(),0,None)?;
                for (n,(fid,rel,hash,_)) in files.iter().enumerate(){
                    if service.cancelled(&job_id)?{return Err(Error::Conflict("cancelled".into()));}
                    let c=service.db.lock()?;let src=crate::library::disk_path(&c,fid,true)?;let dst=hh_core::paths::jail_join(&dest,rel)?;
                    if dst.exists(){if hh_transfer::hash_file(&std::fs::File::open(&dst)?)?!=*hash{return Err(Error::Conflict("move destination contains different content".into()));}}
                    else {crate::verified_copy(&src,&dst,hash)?;}drop(c);
                    service.job_progress(&job_id,"running",files.len(),n+1,None)?;
                }
                let mut config=service.cfg.clone();config.library_root=new.clone();config.ensure_dirs()?;
                let encoded=serde_json::to_string(&config).map_err(|e|Error::Internal(e.to_string()))?;
                {let c=service.db.lock()?;c.execute("INSERT INTO library_moves(job_id,old_root,new_root,config_json,state) VALUES(?1,?2,?3,?4,'switching')",params![job_id,service.cfg.library_root.to_string_lossy(),new.to_string_lossy(),encoded]).map_err(db_e)?;}
                service.finish_library_switch(&job_id,&config)?;
                Ok(json!({"restart_required":true,"originals_preserved":true,"path":new}))
            })();service.finish_job(&job_id,result);
        });self.job(&id)
    }

    fn finish_library_switch(&self,job_id:&str,config:&hh_core::Config)->Result<()> {
        // Durable SQLite intent makes either cross-file commit order recoverable.
        let path=config.data_dir.join("config.json");let temp=config.data_dir.join(format!("config-{}.tmp",ulid::Ulid::new()));
        let mut out=std::fs::OpenOptions::new().create_new(true).write(true).open(&temp)?;use std::io::Write;
        out.write_all(&serde_json::to_vec_pretty(config).map_err(|e|Error::Internal(e.to_string()))?)?;out.sync_all()?;drop(out);
        if path.exists(){crate::atomic_replace(&temp,&path)?;}else{crate::publish_file(&temp,&path)?;}
        let mut c=self.db.lock()?;let tx=c.transaction().map_err(db_e)?;
        tx.execute("UPDATE storage_roots SET path=?1 WHERE kind='library' AND is_active=1",params![config.library_dir().to_string_lossy()]).map_err(db_e)?;
        tx.execute("UPDATE library_moves SET state='completed' WHERE job_id=?1",params![job_id]).map_err(db_e)?;
        tx.execute("UPDATE jobs SET status='completed',result=?2,updated_at=?3 WHERE id=?1",params![job_id,json!({"restart_required":true,"originals_preserved":true,"path":config.library_root}).to_string(),now_ms()]).map_err(db_e)?;tx.commit().map_err(db_e)?;Ok(())
    }
    pub fn recover_library_moves(&self)->Result<hh_core::Config> {
        let moves:Vec<(String,String)>={let c=self.db.lock()?;let mut st=c.prepare("SELECT job_id,config_json FROM library_moves WHERE state='switching'").map_err(db_e)?;let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
        let mut config=self.cfg.clone();
        for (id,encoded) in moves {config=serde_json::from_str(&encoded).map_err(|e|Error::Db(e.to_string()))?;self.finish_library_switch(&id,&config)?;}
        Ok(config)
    }

    /// Startup recovery makes finished disk operations visible and marks
    /// unfinished jobs honestly, without deleting their verified copies.
    pub fn recover_storage_jobs(&self)->Result<()> {
        let mut c=self.db.lock()?;
        let ops:Vec<(String,String,String,String,String)>= {let mut st=c.prepare("SELECT id,file_id,kind,new_path,payload FROM file_operation_journal WHERE state='prepared'").map_err(db_e)?;let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(db_e)?;rows.collect::<std::result::Result<_,_>>().map_err(db_e)?};
        for (jid,fid,kind,new,payload) in ops {
            let value:Value=serde_json::from_str(&payload).map_err(|e|Error::Db(e.to_string()))?;
            let path=Path::new(&new);if !path.is_file(){continue;}
            let f=std::fs::File::open(path)?;if hh_transfer::hash_file(&f)?!=value["hash"].as_str().unwrap_or(""){continue;}
            let tx=c.transaction().map_err(db_e)?;
            if kind=="import"{insert_import(&tx,&fid,&value)?;insert_chunks(&tx,&fid,&f)?;}
            else if kind=="move"{tx.execute("UPDATE files SET name=?2,rel_path=?3 WHERE id=?1",params![fid,value["name"].as_str(),value["rel_path"].as_str()]).map_err(db_e)?;}
            tx.execute("UPDATE file_operation_journal SET state='done' WHERE id=?1",params![jid]).map_err(db_e)?;tx.commit().map_err(db_e)?;
        }
        c.execute("UPDATE jobs SET status='interrupted',error='Interrupted; verified completed copies preserved. Retry the operation.',updated_at=?1 WHERE status IN ('queued','running')",params![now_ms()]).map_err(db_e)?;Ok(())
    }
}

fn insert_import(c:&rusqlite::Connection,id:&str,v:&Value)->Result<()> {
    c.execute("INSERT OR IGNORE INTO files(id,root_id,rel_path,name,category,mime,size,hash,source_mode,created_at,last_verified_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",params![id,v["root_id"].as_str(),v["rel_path"].as_str(),v["name"].as_str(),v["category"].as_str(),v["mime"].as_str(),v["size"].as_i64(),v["hash"].as_str(),v["source_mode"].as_str(),now_ms()]).map_err(db_e)?;Ok(())
}
fn insert_chunks(c:&rusqlite::Connection,id:&str,f:&std::fs::File)->Result<()> {
    use std::io::{Read,Seek};let mut input=f.try_clone()?;input.rewind()?;let mut buf=vec![0;4194304];let mut idx=0;
    loop{let mut n=0;while n<buf.len(){let read=input.read(&mut buf[n..])?;if read==0{break;}n+=read;}if n==0 && idx>0{break;}
        c.execute("INSERT OR REPLACE INTO file_chunks(file_id,idx,hash) VALUES(?1,?2,?3)",params![id,idx,blake3::hash(&buf[..n]).to_hex().to_string()]).map_err(db_e)?;idx+=1;if n<buf.len(){break;}}
    Ok(())
}
fn discover(paths:&[String])->Result<Vec<PathBuf>> {
    if paths.is_empty()||paths.len()>100{return Err(Error::BadRequest("select 1–100 import paths".into()));}
    let mut stack:Vec<PathBuf>=paths.iter().map(PathBuf::from).collect();let mut files=Vec::new();
    while let Some(path)=stack.pop(){let meta=std::fs::symlink_metadata(&path)?;if meta.file_type().is_symlink(){continue;}if meta.is_file(){files.push(path.canonicalize()?);}else if meta.is_dir(){for entry in std::fs::read_dir(path)?{let entry=entry?;if entry.file_name().to_string_lossy().starts_with(".hh-"){continue;}stack.push(entry.path());}}
        if files.len()+stack.len()>1_000_000{return Err(Error::TooLarge("import scan contains too many entries".into()));}}
    files.sort();files.dedup();Ok(files)
}
fn mime_for(name:&str)->&'static str {match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str(){"jpg"|"jpeg"=>"image/jpeg","png"=>"image/png","gif"=>"image/gif","webp"=>"image/webp","heic"=>"image/heic","mp4"=>"video/mp4","mov"=>"video/quicktime","mp3"=>"audio/mpeg","wav"=>"audio/wav","flac"=>"audio/flac","m4a"=>"audio/mp4","pdf"=>"application/pdf","txt"|"md"=>"text/plain",_=>"application/octet-stream"}}
