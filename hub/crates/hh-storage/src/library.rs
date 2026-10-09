//! Library browsing/search (API_SPEC §6). Clients see logical categories,
//! never raw Windows paths (FR-4.1).

use hh_core::error::{Error, Result};
use hh_core::types::{FileObject, Page};
use rusqlite::params;

use crate::StorageService;

impl StorageService {
    /// Legacy timestamp cursor remains accepted; new clients should keep the
    /// opaque cursor returned by list_files_page to retain tie breaks.
    pub fn list_files(&self,category:Option<&str>,query:Option<&str>,cursor:Option<i64>,limit:u32)->Result<Page<FileObject>> {
        let legacy=cursor.map(|n|n.to_string());
        self.list_files_page(category,query,legacy.as_deref(),None,limit)
    }

    pub fn list_files_page(&self,category:Option<&str>,query:Option<&str>,cursor:Option<&str>,sort:Option<&str>,limit:u32)->Result<Page<FileObject>> {
        let limit=limit.clamp(1,500) as i64;
        let sort=sort.unwrap_or("newest");
        let (column,direction,compare)=match sort {
            "newest"=>("created_at","DESC","<"), "oldest"=>("created_at","ASC",">"),
            "name"=>("name","ASC",">"),"size"=>("size","DESC","<"),
            _=>return Err(Error::BadRequest("invalid sort".into()))
        };
        let mut sql=String::from("SELECT id,name,category,mime,size,hash,created_at,modified_at,rel_path,last_verified_at FROM files WHERE deleted_at IS NULL");
        let mut vals:Vec<Box<dyn rusqlite::ToSql>>=Vec::new();
        if let Some(cat)=category {sql.push_str(" AND category=?");vals.push(Box::new(cat.to_string()));}
        if let Some(q)=query {sql.push_str(" AND name LIKE ? ESCAPE '\\'");vals.push(Box::new(format!("%{}%",q.replace('\\',"\\\\").replace('%',"\\%").replace('_',"\\_"))));}
        if let Some(cur)=cursor {
            if let Ok((cursor_sort,value,id))=serde_json::from_str::<(String,serde_json::Value,String)>(cur) {
                if cursor_sort!=sort {return Err(Error::BadRequest("cursor sort changed".into()));}
                sql.push_str(&format!(" AND ({column} {compare} ? OR ({column} = ? AND id {compare} ?))"));
                if column=="name" {
                    let v=value.as_str().ok_or_else(||Error::BadRequest("cursor".into()))?.to_string();
                    vals.push(Box::new(v.clone()));vals.push(Box::new(v));
                } else {
                    let v=value.as_i64().ok_or_else(||Error::BadRequest("cursor".into()))?;
                    vals.push(Box::new(v));vals.push(Box::new(v));
                }
                vals.push(Box::new(id));
            } else if let Ok(ts)=cur.parse::<i64>() {sql.push_str(" AND created_at < ?");vals.push(Box::new(ts));}
            else {return Err(Error::BadRequest("invalid cursor".into()));}
        }
        sql.push_str(&format!(" ORDER BY {column} {direction},id {direction} LIMIT ?"));vals.push(Box::new(limit+1));
        let c=self.db.lock()?;
        let mut st=c.prepare(&sql).map_err(db_e)?;
        let refs:Vec<&dyn rusqlite::ToSql>=vals.iter().map(|v|v.as_ref()).collect();
        let mut items=st.query_map(refs.as_slice(),file_object).map_err(db_e)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_e)?;
        let next_cursor=if items.len()>limit as usize {
            items.truncate(limit as usize);
            items.last().map(|f|serde_json::json!([sort,match column {"name"=>serde_json::json!(f.name),"size"=>serde_json::json!(f.size),_=>serde_json::json!(f.created_at)},f.id]).to_string())
        } else {None};
        drop(st);
        drop(c);
        for file in &mut items { file.second_copy_status=self.file_second_copy_status(&file.id,&file.hash,file.size)?; }
        Ok(Page{items,next_cursor})
    }

    pub fn get_file(&self, id: &str) -> Result<FileObject> {
        let c = self.db.lock()?;
        let mut file=c.query_row(
            "SELECT id,name,category,mime,size,hash,created_at,modified_at,rel_path,last_verified_at
             FROM files WHERE id=?1 AND deleted_at IS NULL",
            params![id],
            file_object,
        )
        .map_err(|_| Error::NotFound(format!("file {id}")))?;
        drop(c);
        file.second_copy_status=self.file_second_copy_status(&file.id,&file.hash,file.size)?;
        Ok(file)
    }

    /// Absolute path on disk for content serving; jailed to the library.
    pub fn file_disk_path(&self, id: &str) -> Result<std::path::PathBuf> {
        let c = self.db.lock()?;
        disk_path(&c,id,false)
    }

    /// Chunk manifest for download verification (API_SPEC §6).
    pub fn file_manifest(&self, id: &str) -> Result<Vec<(u64, String)>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT idx, hash FROM file_chunks WHERE file_id=?1 ORDER BY idx")
            .map_err(db_e)?;
        let rows = st
            .query_map(params![id], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?)))
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        if rows.is_empty() {
            return Err(Error::NotFound(format!("manifest for {id}")));
        }
        Ok(rows)
    }

    pub fn rename_file(&self,id:&str,new_name:&str)->Result<()> {
        let name=hh_core::paths::sanitize_component(new_name)?;
        let c=self.db.lock()?;
        let rel:String=c.query_row("SELECT rel_path FROM files WHERE id=?1",params![id],|r|r.get(0)).map_err(db_e)?;
        drop(c);
        let parent=std::path::Path::new(&rel).parent().unwrap_or(std::path::Path::new(""));
        self.move_file(id,&parent.join(name).to_string_lossy().replace('\\',"/"))
    }

    pub fn move_file(&self,id:&str,rel_path:&str)->Result<()> {
        let rel=hh_core::paths::sanitize_rel_path(rel_path)?;
        let mut c=self.db.lock()?;
        assert_mutable(&c,id)?;
        let old=disk_path(&c,id,false)?;
        let (root,hash):(String,String)=c.query_row("SELECT r.path,f.hash FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE f.id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_e)?;
        let new=hh_core::paths::jail_join(std::path::Path::new(&root),&rel)?;
        if new==old {return Ok(());}
        if new.exists() {return Err(Error::Conflict("destination exists".into()));}
        let name=new.file_name().ok_or_else(||Error::BadRequest("filename".into()))?.to_string_lossy().to_string();
        if let Some(p)=new.parent(){std::fs::create_dir_all(p)?;}
        let jid=ulid::Ulid::new().to_string();
        let payload=serde_json::json!({"rel_path":rel,"name":name,"hash":hash}).to_string();
        c.execute("INSERT INTO file_operation_journal(id,file_id,kind,old_path,new_path,payload,state,created_at) VALUES(?1,?2,'move',?3,?4,?5,'prepared',?6)",params![jid,id,old.to_string_lossy(),new.to_string_lossy(),payload,hh_core::time::now_ms()]).map_err(db_e)?;
        super::publish_file(&old,&new)?;
        let tx=c.transaction().map_err(db_e)?;
        tx.execute("UPDATE files SET name=?2,rel_path=?3 WHERE id=?1",params![id,name,rel]).map_err(db_e)?;
        tx.execute("UPDATE file_operation_journal SET state='done' WHERE id=?1",params![jid]).map_err(db_e)?;
        tx.commit().map_err(db_e)?;
        Ok(())
    }

    pub fn copy_file(&self,id:&str,rel_path:&str,name:Option<&str>)->Result<FileObject> {
        let original=self.get_file(id)?;
        let src=self.file_disk_path(id)?;
        let rel=if let Some(name)=name {
            std::path::Path::new(rel_path).join(hh_core::paths::sanitize_component(name)?).to_string_lossy().replace('\\',"/")
        } else {rel_path.to_string()};
        let new_id=self.import_one(&src,"copy",Some(&rel),Some(&original.hash))?;
        self.get_file(&new_id)
    }

}

pub(crate) fn db_e(e: rusqlite::Error) -> Error {
    Error::Db(e.to_string())
}

fn file_object(r: &rusqlite::Row<'_>) -> std::result::Result<FileObject, rusqlite::Error> {
    let rel: String = r.get(8)?;
    let path = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    Ok(FileObject {
        id: r.get(0)?,
        name: r.get(1)?,
        category: r.get(2)?,
        mime: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        hash: r.get(5)?,
        created_at: r.get(6)?,
        modified_at: r.get(7)?,
        path,
        last_verified_at: r.get(9)?,
        second_copy_status: String::new(),
    })
}

pub(crate) fn disk_path(c:&rusqlite::Connection,id:&str,include_trash:bool)->Result<std::path::PathBuf> {
    let (root,rel,deleted):(String,String,Option<i64>)=c.query_row("SELECT r.path,f.rel_path,f.deleted_at FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE f.id=?1 AND r.is_active=1",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|Error::NotFound(format!("file {id}")))?;
    if !include_trash && deleted.is_some() {return Err(Error::NotFound(format!("file {id}")));}
    hh_core::paths::jail_join(std::path::Path::new(&root),&rel)
}

pub(crate) fn assert_mutable(c:&rusqlite::Connection,id:&str)->Result<()> {
    let moving:i64=c.query_row("SELECT COUNT(*) FROM jobs WHERE kind='library_move' AND status IN ('queued','running')",[],|r|r.get(0)).map_err(db_e)?;
    if moving>0{return Err(Error::Conflict("library move is in progress".into()));}
    let mode:String=c.query_row("SELECT source_mode FROM files WHERE id=?1",params![id],|r|r.get(0)).map_err(db_e)?;
    if mode=="keep_in_place" {return Err(Error::Conflict("externally managed file: originals cannot be modified".into()));}
    let pinned:i64=c.query_row("SELECT COUNT(*) FROM cleanup_lease_items i JOIN cleanup_leases l ON l.id=i.lease_id WHERE i.file_id=?1 AND l.state='active'",params![id],|r|r.get(0)).map_err(db_e)?;
    if pinned>0 {return Err(Error::Conflict("file protected by pending phone cleanup".into()));}
    let staged:i64=c.query_row("SELECT COUNT(*) FROM relay_delivery WHERE file_id=?1 AND status='pending'",params![id],|r|r.get(0)).map_err(db_e)?;
    if staged>0 {return Err(Error::Conflict("file protected by pending delivery; cancel delivery first".into()));}
    Ok(())
}
