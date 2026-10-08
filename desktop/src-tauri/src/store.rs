//! Private per-user state. Hub files and pairing credentials never enter this database.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

pub type Result<T> = std::result::Result<T, String>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record { pub id: String, pub kind: String, pub title: String, pub body: String, pub updated_at: i64, pub deleted_at: Option<i64> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSessionTab { pub url: String, pub isolated: bool }
pub struct Store { pub db: Mutex<Connection>, pub root: PathBuf }
pub fn now() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|v| v.as_millis() as i64).unwrap_or(0) }

pub fn private_directory(path: &Path) -> Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.is_dir() || meta.file_type().is_symlink() { return Err("Private storage cannot be a linked folder".into()); }
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 { return Err("Private storage cannot be a linked folder".into()); }
        }
    }
    std::fs::create_dir_all(path).map_err(|_| "Could not create private app storage")?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|_| "Could not protect app storage")?;
    }
    #[cfg(windows)] {
        let output = std::process::Command::new(r"C:\Windows\System32\whoami.exe").args(["/user", "/fo", "csv", "/nh"]).output().map_err(|_| "Could not identify Windows user")?;
        if !output.status.success() { return Err("Could not identify Windows user".into()); }
        let text = String::from_utf8_lossy(&output.stdout);
        let sid = text.trim().split(',').nth(1).map(|s| s.trim_matches('"')).ok_or("Could not identify Windows user")?;
        if !sid.starts_with("S-1-") || !sid.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S') { return Err("Invalid Windows identity".into()); }
        // Replace the complete DACL atomically. Do not briefly restore inherited
        // access, and do not preserve stale explicit grants to other users.
        let script = "$acl = New-Object System.Security.AccessControl.DirectorySecurity; $acl.SetSecurityDescriptorSddlForm(('D:P(A;OICI;FA;;;' + $env:HH_PRIVATE_SID + ')')); Set-Acl -LiteralPath $env:HH_PRIVATE_DIR -AclObject $acl -ErrorAction Stop";
        let status = std::process::Command::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).env("HH_PRIVATE_SID",sid).env("HH_PRIVATE_DIR",path).output().map_err(|_| "Could not protect app storage")?;
        if !status.status.success() { return Err("Could not protect app storage".into()); }
    }
    Ok(())
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        private_directory(&root)?;
        for name in ["desktop.db", "desktop.db-wal", "desktop.db-shm"] { reject_link(&root.join(name))?; }
        let db = Connection::open(root.join("desktop.db")).map_err(|_| "Could not open private app storage")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000; CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY,kind TEXT NOT NULL,title TEXT NOT NULL,body TEXT NOT NULL,updated_at INTEGER NOT NULL,deleted_at INTEGER); CREATE INDEX IF NOT EXISTS records_kind ON records(kind,deleted_at,updated_at); CREATE TABLE IF NOT EXISTS browser_session(position INTEGER PRIMARY KEY,url TEXT NOT NULL,isolated INTEGER NOT NULL);").map_err(|_| "Could not prepare private app storage")?;
        let store=Self { db: Mutex::new(db), root };
        for row in store.list("download","",false)? {
            if let Ok(mut body)=serde_json::from_str::<serde_json::Value>(&row.body) {if body["status"]=="downloading" {body["status"]=serde_json::json!("failed");body["reason"]=serde_json::json!("interrupted");store.save(Some(row.id),row.kind,row.title,body.to_string())?;}}
        }
        Ok(store)
    }
    pub fn list(&self, kind: &str, query: &str, trash: bool) -> Result<Vec<Record>> {
        validate_kind(kind)?;
        if query.len() > 512 { return Err("Search is too long".into()); }
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        let mut stmt = db.prepare("SELECT id,kind,title,body,updated_at,deleted_at FROM records WHERE kind=?1 AND ((?2=1 AND deleted_at IS NOT NULL) OR (?2=0 AND deleted_at IS NULL)) AND (instr(lower(title),lower(?3))>0 OR instr(lower(body),lower(?3))>0) ORDER BY updated_at DESC LIMIT 1500").map_err(|_| "Could not search app storage")?;
        let rows = stmt.query_map(params![kind, trash, query], |r| Ok(Record { id:r.get(0)?, kind:r.get(1)?, title:r.get(2)?, body:r.get(3)?, updated_at:r.get(4)?, deleted_at:r.get(5)? })).map_err(|_| "Could not read app storage")?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(|_| "Could not read app storage".into())
    }
    pub fn save(&self, id: Option<String>, kind: String, title: String, body: String) -> Result<Record> {
        let record = Record { id: id.unwrap_or_else(|| ulid::Ulid::new().to_string()), kind, title, body, updated_at:now(), deleted_at:None };
        validate_record(&record)?;
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM records WHERE id=?1)",params![record.id],|r|r.get(0)).map_err(|_|"Could not read app storage")?;
        if !exists {
            let portable=["note","playlist","bookmark"].contains(&record.kind.as_str());
            let count:i64 = if portable {db.query_row("SELECT COUNT(*) FROM records WHERE kind IN ('note','playlist','bookmark')",[],|r|r.get(0))} else {db.query_row("SELECT COUNT(*) FROM records WHERE kind=?1",params![record.kind],|r|r.get(0))}.map_err(|_|"Could not read app storage")?;
            let limit=if portable {1500}else{500};
            if count >= limit {return Err("Private app storage is full. Export your data before adding more items.".into());}
        }
        // Never allow an update to change a record's type or silently restore trash.
        let changed = db.execute("INSERT INTO records(id,kind,title,body,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,updated_at=excluded.updated_at WHERE records.kind=excluded.kind AND records.deleted_at IS NULL", params![record.id,record.kind,record.title,record.body,record.updated_at]).map_err(|_| "Could not save app data")?;
        if changed != 1 {return Err("Item is in trash or belongs to another app".into());}
        Ok(record)
    }
    pub fn trash(&self, id: &str, restore: bool) -> Result<()> {
        validate_id(id)?;
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        let changed = db.execute("UPDATE records SET deleted_at=?1,updated_at=?2 WHERE id=?3", params![if restore {None} else {Some(now())},now(),id]).map_err(|_| "Could not update trash")?;
        if changed != 1 { return Err("Item no longer exists".into()); }
        Ok(())
    }
    pub fn purge(&self,id:&str)->Result<()> {
        validate_id(id)?;
        let db=self.db.lock().map_err(|_|"App storage is busy")?;
        let changed=db.execute("DELETE FROM records WHERE id=?1 AND deleted_at IS NOT NULL AND kind IN ('note','playlist','bookmark')",params![id]).map_err(|_|"Could not empty private trash")?;
        if changed!=1{return Err("Only items in private trash can be permanently removed".into());}Ok(())
    }
    pub fn clear_browser_history(&self)->Result<()> {
        let mut db=self.db.lock().map_err(|_|"App storage is busy")?;
        let tx=db.transaction().map_err(|_|"Could not clear browser history")?;
        tx.execute("DELETE FROM records WHERE kind='browser'",[]).map_err(|_|"Could not clear browser history")?;
        tx.execute("DELETE FROM browser_session",[]).map_err(|_|"Could not clear browser session")?;
        tx.commit().map_err(|_|"Could not clear browser data")?;
        Ok(())
    }
    pub fn browser_session(&self)->Result<Vec<BrowserSessionTab>> {
        let db=self.db.lock().map_err(|_|"App storage is busy")?;
        let mut stmt=db.prepare("SELECT url,isolated FROM browser_session ORDER BY position LIMIT 12").map_err(|_|"Could not read browser session")?;
        let rows=stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).map_err(|_|"Could not read browser session")?;
        rows.map(|row| {
            let (url,isolated)=row.map_err(|_|"Could not read browser session")?;
            crate::browser::public_url(&url)?;
            if isolated!=0 && isolated!=1 {return Err("Invalid browser session".into());}
            Ok(BrowserSessionTab {url,isolated:isolated==1})
        }).collect()
    }
    pub fn save_browser_session(&self,tabs:&[BrowserSessionTab])->Result<()> {
        if tabs.len()>12 {return Err("Too many browser tabs".into());}
        for tab in tabs {crate::browser::public_url(&tab.url)?;}
        let mut db=self.db.lock().map_err(|_|"App storage is busy")?;
        let tx=db.transaction().map_err(|_|"Could not save browser session")?;
        tx.execute("DELETE FROM browser_session",[]).map_err(|_|"Could not save browser session")?;
        for (position,tab) in tabs.iter().enumerate() {
            tx.execute("INSERT INTO browser_session(position,url,isolated) VALUES(?1,?2,?3)",params![position as i64,&tab.url,tab.isolated]).map_err(|_|"Could not save browser session")?;
        }
        tx.commit().map_err(|_|"Could not save browser session")?;
        Ok(())
    }
    pub fn export(&self) -> Result<String> {
        let mut records = Vec::new();
        for kind in ["note","playlist","bookmark"] { records.extend(self.list(kind,"",false)?); records.extend(self.list(kind,"",true)?); }
        if records.len()>1500 || records.iter().map(|r|r.body.len()+r.title.len()).sum::<usize>()>8*1024*1024 {return Err("Private export exceeds 8 MB. Export individual notes as Markdown or text.".into());}
        let text=serde_json::to_string_pretty(&serde_json::json!({"format":"home-hub-private","version":1,"records":records})).map_err(|_| "Could not export app data")?;
        if text.len()>8*1024*1024 {return Err("Private export exceeds 8 MB. Export individual notes as Markdown or text.".into());}Ok(text)
    }
    pub fn import(&self, text: &str) -> Result<usize> {
        if text.len() > 8*1024*1024 { return Err("Import exceeds 8 MB".into()); }
        #[derive(Deserialize)] struct Export { format:String, version:u32, records:Vec<Record> }
        let data: Export = serde_json::from_str(text).map_err(|_| "This is not a Home Hub app export")?;
        if data.format != "home-hub-private" || data.version != 1 || data.records.len()>1500 { return Err("Unsupported app export".into()); }
        for r in &data.records { validate_record(r)?; if !["note","playlist","bookmark"].contains(&r.kind.as_str()) { return Err("Export contains unsupported data".into()); } }
        let mut db = self.db.lock().map_err(|_| "App storage is busy")?;
        let tx = db.transaction().map_err(|_| "Could not start import")?;
        let existing:i64=tx.query_row("SELECT COUNT(*) FROM records WHERE kind IN ('note','playlist','bookmark')",[],|r|r.get(0)).map_err(|_|"Could not read app storage")?;
        let mut count=0;
        for r in data.records { count += tx.execute("INSERT OR IGNORE INTO records VALUES(?1,?2,?3,?4,?5,?6)", params![r.id,r.kind,r.title,r.body,r.updated_at,r.deleted_at]).map_err(|_| "Could not import app data")?; }
        if existing + count as i64 > 1500 { return Err("Import would exceed private app storage limits".into()); }
        tx.commit().map_err(|_| "Could not finish import")?;
        Ok(count)
    }
}
fn reject_link(path:&Path)->Result<()> {
    if let Ok(meta)=std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {return Err("Private storage cannot use linked files".into());}
        #[cfg(windows)] {use std::os::windows::fs::MetadataExt; if meta.file_attributes() & 0x400 != 0 {return Err("Private storage cannot use linked files".into());}}
    } Ok(())
}
fn validate_kind(kind:&str)->Result<()> { if ["note","playlist","bookmark","browser","preference","download"].contains(&kind) { Ok(()) } else { Err("Unknown app data type".into()) } }
pub fn validate_id(id:&str)->Result<()> { if id.len()==26 && id.chars().all(|c| c.is_ascii_alphanumeric()) { Ok(()) } else { Err("Invalid item ID".into()) } }
fn validate_record(r:&Record)->Result<()> {
    validate_id(&r.id)?; validate_kind(&r.kind)?;
    if r.title.len()>512 || r.body.len()>1024*1024 { return Err("Item is too large".into()); }
    if r.kind != "note" {
        let value=serde_json::from_str::<serde_json::Value>(&r.body).map_err(|_| "Invalid app data")?;
        if !value.is_object() {return Err("Invalid app data".into());}
        if r.kind=="playlist" {
            let ids=value["ids"].as_array().ok_or("Invalid playlist")?;
            if ids.len()>1500 {return Err("Playlist is too large".into());}
            for id in ids {validate_id(id.as_str().ok_or("Invalid playlist item")?)?;}
        }
        if ["bookmark","browser"].contains(&r.kind.as_str()) {crate::browser::public_url(value["url"].as_str().ok_or("Invalid website address")?)?;}
    }
    Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    fn store() -> Store { Store::open(std::env::temp_dir().join(format!("hh-desktop-{}",ulid::Ulid::new()))).unwrap() }
    #[test] fn note_roundtrip_trash_and_restore() { let s=store(); let n=s.save(None,"note".into(),"Shopping".into(),"Milk".into()).unwrap(); assert_eq!(s.list("note","milk",false).unwrap().len(),1); s.trash(&n.id,false).unwrap(); assert!(s.list("note","",false).unwrap().is_empty()); s.trash(&n.id,true).unwrap(); assert_eq!(s.list("note","",false).unwrap().len(),1); std::fs::remove_dir_all(&s.root).ok(); }
    #[test] fn import_is_transactional_and_preserves_existing_notes() { let s=store(); s.save(None,"note".into(),"Original".into(),"Private".into()).unwrap(); let data=s.export().unwrap(); assert_eq!(s.import(&data).unwrap(),0); assert!(s.import("{\"format\":\"home-hub-private\",\"version\":1,\"records\":[{\"id\":\"bad\",\"kind\":\"note\",\"title\":\"x\",\"body\":\"x\",\"updated_at\":0,\"deleted_at\":null}]}").is_err()); assert_eq!(s.list("note","",false).unwrap().len(),1); std::fs::remove_dir_all(&s.root).ok(); }
    #[test] fn saves_cannot_change_type_or_restore_trashed_data() {
        let s=store();let n=s.save(None,"note".into(),"Safe".into(),"Original".into()).unwrap();
        assert!(s.save(Some(n.id.clone()),"playlist".into(),"Overwrite".into(),"{\"ids\":[]}".into()).is_err());
        s.trash(&n.id,false).unwrap();assert!(s.save(Some(n.id.clone()),"note".into(),"Overwrite".into(),"Lost".into()).is_err());
        let rows=s.list("note","",true).unwrap();assert_eq!(rows[0].body,"Original");
        drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
    #[test] fn permanent_removal_requires_private_trash() {
        let s=store();let n=s.save(None,"note".into(),"Safe".into(),"Keep".into()).unwrap();assert!(s.purge(&n.id).is_err());s.trash(&n.id,false).unwrap();s.purge(&n.id).unwrap();assert!(s.list("note","",true).unwrap().is_empty());drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
    #[test] fn malformed_playlist_and_private_bookmark_imports_are_rejected_atomically() {
        let s=store();let note=Record{id:ulid::Ulid::new().to_string(),kind:"note".into(),title:"Uncommitted".into(),body:"Secret".into(),updated_at:now(),deleted_at:None};
        for (kind,body) in [("playlist","{\"ids\":17}"),("bookmark","{\"url\":\"https://127.0.0.1:47801\"}")] {
            let hostile=Record{id:ulid::Ulid::new().to_string(),kind:kind.into(),title:"Unsafe".into(),body:body.into(),updated_at:now(),deleted_at:None};
            let export=serde_json::json!({"format":"home-hub-private","version":1,"records":[note,hostile]}).to_string();assert!(s.import(&export).is_err());assert!(s.list("note","",false).unwrap().is_empty());
        }
        drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
    #[test] fn portable_exports_do_not_include_preferences_browser_sessions_or_download_paths() {
        let s=store();s.save(None,"preference".into(),"interface".into(),"{\"private\":\"NOT_EXPORTED\"}".into()).unwrap();s.save(None,"browser".into(),"session".into(),"{\"url\":\"https://example.com/NOT_EXPORTED\"}".into()).unwrap();s.save_browser_session(&[BrowserSessionTab {url:"https://example.com/NOT_EXPORTED".into(),isolated:false}]).unwrap();s.save(None,"download".into(),"download".into(),"{\"path\":\"NOT_EXPORTED\"}".into()).unwrap();s.save(None,"note".into(),"Note".into(),"Markdown".into()).unwrap();assert!(!s.export().unwrap().contains("NOT_EXPORTED"));
        drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
    #[test] fn clearing_browser_history_removes_active_and_trashed_rows_only() {
        let s=store();
        let active=s.save(None,"browser".into(),"Page A".into(),"{\"url\":\"https://example.com/a\"}".into()).unwrap();
        let trashed=s.save(None,"browser".into(),"Page B".into(),"{\"url\":\"https://example.com/b\"}".into()).unwrap();
        s.trash(&trashed.id,false).unwrap();
        s.save(None,"note".into(),"Keep".into(),"Private".into()).unwrap();
        s.save(None,"bookmark".into(),"Keep".into(),"{\"url\":\"https://example.com/bookmark\"}".into()).unwrap();
        s.save(None,"download".into(),"Keep".into(),"{\"status\":\"complete\"}".into()).unwrap();
        s.save_browser_session(&[BrowserSessionTab {url:"https://example.com/reopen".into(),isolated:false}]).unwrap();
        s.clear_browser_history().unwrap();
        assert!(s.list("browser","",false).unwrap().is_empty());
        assert!(s.list("browser","",true).unwrap().is_empty());
        assert!(s.browser_session().unwrap().is_empty());
        assert_eq!(s.list("note","",false).unwrap().len(),1);
        assert_eq!(s.list("bookmark","",false).unwrap().len(),1);
        assert_eq!(s.list("download","",false).unwrap().len(),1);
        assert!(!s.db.lock().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM records WHERE id=?1)",params![active.id],|r|r.get::<_,bool>(0)).unwrap());
        drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
    #[test] fn browser_session_roundtrip_replaces_tabs_without_restoring_grants() {
        let s=store();
        let previous=vec![BrowserSessionTab {url:"https://example.com/first".into(),isolated:false},BrowserSessionTab {url:"https://www.youtube.com/watch?v=1".into(),isolated:true}];
        s.save_browser_session(&previous).unwrap();
        assert_eq!(s.browser_session().unwrap(),previous);
        assert!(s.save_browser_session(&[BrowserSessionTab {url:"https://127.0.0.1:47801".into(),isolated:false}]).is_err());
        assert_eq!(s.browser_session().unwrap(),previous);
        s.save_browser_session(&previous[1..]).unwrap();
        assert_eq!(s.browser_session().unwrap(),previous[1..]);
        drop(s.db.into_inner().unwrap());std::fs::remove_dir_all(&s.root).ok();
    }
}
