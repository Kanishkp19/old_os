//! Private per-user state. Hub files and pairing credentials never enter this database.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

pub type Result<T> = std::result::Result<T, String>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record { pub id: String, pub kind: String, pub title: String, pub body: String, pub updated_at: i64, pub deleted_at: Option<i64> }
pub struct Store { pub db: Mutex<Connection>, pub root: PathBuf }
pub fn now() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|v| v.as_millis() as i64).unwrap_or(0) }

pub fn private_directory(path: &Path) -> Result<()> {
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
        let grant = format!("*{sid}:(OI)(CI)F");
        let status = std::process::Command::new(r"C:\Windows\System32\icacls.exe").arg(path).args(["/inheritance:r", "/grant:r", &grant]).output().map_err(|_| "Could not protect app storage")?;
        if !status.status.success() { return Err("Could not protect app storage".into()); }
    }
    Ok(())
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        private_directory(&root)?;
        let db = Connection::open(root.join("desktop.db")).map_err(|_| "Could not open private app storage")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000; CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY,kind TEXT NOT NULL,title TEXT NOT NULL,body TEXT NOT NULL,updated_at INTEGER NOT NULL,deleted_at INTEGER); CREATE INDEX IF NOT EXISTS records_kind ON records(kind,deleted_at,updated_at);").map_err(|_| "Could not prepare private app storage")?;
        Ok(Self { db: Mutex::new(db), root })
    }
    pub fn list(&self, kind: &str, query: &str, trash: bool) -> Result<Vec<Record>> {
        validate_kind(kind)?;
        if query.len() > 512 { return Err("Search is too long".into()); }
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        let mut stmt = db.prepare("SELECT id,kind,title,body,updated_at,deleted_at FROM records WHERE kind=?1 AND ((?2=1 AND deleted_at IS NOT NULL) OR (?2=0 AND deleted_at IS NULL)) AND (instr(lower(title),lower(?3))>0 OR instr(lower(body),lower(?3))>0) ORDER BY updated_at DESC LIMIT 500").map_err(|_| "Could not search app storage")?;
        let rows = stmt.query_map(params![kind, trash, query], |r| Ok(Record { id:r.get(0)?, kind:r.get(1)?, title:r.get(2)?, body:r.get(3)?, updated_at:r.get(4)?, deleted_at:r.get(5)? })).map_err(|_| "Could not read app storage")?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(|_| "Could not read app storage".into())
    }
    pub fn save(&self, id: Option<String>, kind: String, title: String, body: String) -> Result<Record> {
        let record = Record { id: id.unwrap_or_else(|| ulid::Ulid::new().to_string()), kind, title, body, updated_at:now(), deleted_at:None };
        validate_record(&record)?;
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        // Never allow an update to change a record's type or silently restore trash.
        db.execute("INSERT INTO records(id,kind,title,body,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,updated_at=excluded.updated_at WHERE records.kind=excluded.kind AND records.deleted_at IS NULL", params![record.id,record.kind,record.title,record.body,record.updated_at]).map_err(|_| "Could not save app data")?;
        Ok(record)
    }
    pub fn trash(&self, id: &str, restore: bool) -> Result<()> {
        validate_id(id)?;
        let db = self.db.lock().map_err(|_| "App storage is busy")?;
        let changed = db.execute("UPDATE records SET deleted_at=?1,updated_at=?2 WHERE id=?3", params![if restore {None} else {Some(now())},now(),id]).map_err(|_| "Could not update trash")?;
        if changed != 1 { return Err("Item no longer exists".into()); }
        Ok(())
    }
    pub fn export(&self) -> Result<String> {
        let mut records = Vec::new();
        for kind in ["note","playlist","bookmark"] { records.extend(self.list(kind,"",false)?); records.extend(self.list(kind,"",true)?); }
        serde_json::to_string_pretty(&serde_json::json!({"format":"home-hub-private","version":1,"records":records})).map_err(|_| "Could not export app data".into())
    }
    pub fn import(&self, text: &str) -> Result<usize> {
        if text.len() > 8*1024*1024 { return Err("Import exceeds 8 MB".into()); }
        #[derive(Deserialize)] struct Export { format:String, version:u32, records:Vec<Record> }
        let data: Export = serde_json::from_str(text).map_err(|_| "This is not a Home Hub app export")?;
        if data.format != "home-hub-private" || data.version != 1 || data.records.len()>1500 { return Err("Unsupported app export".into()); }
        for r in &data.records { validate_record(r)?; if !["note","playlist","bookmark"].contains(&r.kind.as_str()) { return Err("Export contains unsupported data".into()); } }
        let mut db = self.db.lock().map_err(|_| "App storage is busy")?;
        let tx = db.transaction().map_err(|_| "Could not start import")?;
        let mut count=0;
        for r in data.records { count += tx.execute("INSERT OR IGNORE INTO records VALUES(?1,?2,?3,?4,?5,?6)", params![r.id,r.kind,r.title,r.body,r.updated_at,r.deleted_at]).map_err(|_| "Could not import app data")?; }
        tx.commit().map_err(|_| "Could not finish import")?;
        Ok(count)
    }
}
fn validate_kind(kind:&str)->Result<()> { if ["note","playlist","bookmark","browser","preference","download"].contains(&kind) { Ok(()) } else { Err("Unknown app data type".into()) } }
pub fn validate_id(id:&str)->Result<()> { if id.len()==26 && id.chars().all(|c| c.is_ascii_alphanumeric()) { Ok(()) } else { Err("Invalid item ID".into()) } }
fn validate_record(r:&Record)->Result<()> {
    validate_id(&r.id)?; validate_kind(&r.kind)?;
    if r.title.len()>512 || r.body.len()>1024*1024 { return Err("Item is too large".into()); }
    if r.kind != "note" { serde_json::from_str::<serde_json::Value>(&r.body).map_err(|_| "Invalid app data")?; }
    Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    fn store() -> Store { Store::open(std::env::temp_dir().join(format!("hh-desktop-{}",ulid::Ulid::new()))).unwrap() }
    #[test] fn note_roundtrip_trash_and_restore() { let s=store(); let n=s.save(None,"note".into(),"Shopping".into(),"Milk".into()).unwrap(); assert_eq!(s.list("note","milk",false).unwrap().len(),1); s.trash(&n.id,false).unwrap(); assert!(s.list("note","",false).unwrap().is_empty()); s.trash(&n.id,true).unwrap(); assert_eq!(s.list("note","",false).unwrap().len(),1); std::fs::remove_dir_all(&s.root).ok(); }
    #[test] fn import_is_transactional_and_preserves_existing_notes() { let s=store(); s.save(None,"note".into(),"Original".into(),"Private".into()).unwrap(); let data=s.export().unwrap(); assert_eq!(s.import(&data).unwrap(),0); assert!(s.import("{\"format\":\"home-hub-private\",\"version\":1,\"records\":[{\"id\":\"bad\",\"kind\":\"note\",\"title\":\"x\",\"body\":\"x\",\"updated_at\":0,\"deleted_at\":null}]}").is_err()); assert_eq!(s.list("note","",false).unwrap().len(),1); std::fs::remove_dir_all(&s.root).ok(); }
}
