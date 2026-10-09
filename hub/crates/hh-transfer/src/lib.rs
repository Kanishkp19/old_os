//! hh-transfer: chunked, resumable, BLAKE3-verified uploads with atomic
//! finalize (TRD §6, AGENTS.md §2.1).
//!
//! Invariants:
//! - Partial data only ever exists as `<root>/.hh-tmp/<transfer_id>.part`,
//!   preallocated on the SAME VOLUME as the destination so the finalize
//!   rename is atomic.
//! - Every chunk is BLAKE3-verified before it is recorded (T-06).
//! - Finalize = verify whole-file root → fsync → atomic rename → DB commit.
//! - On startup, `.part` files are reconciled against the DB; orphans older
//!   than the TTL are deleted.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use hh_core::error::{Error, Result};
use hh_core::paths;
use hh_core::time::now_ms;
use hh_core::types::{CompleteResponse, CreateTransferRequest, CreateTransferResponse, TransferStatus};
use hh_core::{Config, DEFAULT_CHUNK_SIZE, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE, TRANSFER_IDLE_TTL_MS};
use hh_db::{Db, TransferRow};



#[derive(Clone)]
pub struct TransferEngine {
    db: Db,
    cfg: Config,
    mutations: std::sync::Arc<std::sync::Mutex<()>>,
}

impl TransferEngine {
    pub fn new(db: Db, cfg: Config) -> Self {
        Self { db, cfg, mutations: std::sync::Arc::new(std::sync::Mutex::new(())) }
    }

    /// Create or resume an upload session (API_SPEC §5 `POST /transfers`).
    pub fn create(&self, device_id: &str, req: &CreateTransferRequest) -> Result<CreateTransferResponse> {
        let _guard = self.mutations.lock().map_err(|_| Error::Internal("transfer mutex poisoned".into()))?;
        if req.size > (1u64 << 40) { return Err(Error::TooLarge("max file size 1 TiB".into())); }
        if !["send", "backup"].contains(&req.kind.as_str()) { return Err(Error::BadRequest("unsupported transfer kind".into())); }
        if let Some(hash) = &req.root_hash { validate_hash(hash)?; }
        if let Some(rel) = &req.rel_path { paths::sanitize_rel_path(rel)?; }
        if req.client_item_id.as_ref().is_some_and(|v| v.len() > 1024) { return Err(Error::TooLarge("client item id".into())); }
        if let Some(source) = &req.backup_source_id {
            let c = self.db.lock()?;
            let owned: i64 = c.query_row("SELECT COUNT(*) FROM backup_sources WHERE id=?1 AND device_id=?2 AND enabled=1", rusqlite::params![source,device_id],|r|r.get(0)).map_err(|e| Error::Db(e.to_string()))?;
            if owned != 1 || req.kind != "backup" || req.client_item_id.is_none() { return Err(Error::ForbiddenScope("photos".into())); }
        }
        self.check_activity(device_id)?;
        let name = paths::sanitize_component(&req.name)?;
        let chunk_size = req.chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE).clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        // An empty file has no chunks; it can go straight to whole-file verification.
        let chunk_count = req.size.div_ceil(chunk_size);

        // Idempotency: same client_item_id + size → resume existing session (§6.3).
        if let Some(item) = &req.client_item_id {
            if let Some(t) = self.db.find_resumable(device_id, item, req.size)? {
                let (source,target): (Option<String>,Option<String>) = { let c=self.db.lock()?;c.query_row("SELECT backup_source_id,target_device_id FROM transfers WHERE id=?1",[&t.id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|Error::Db(e.to_string()))? };
                let same_hash = match (&t.expected_root_hash,&req.root_hash) { (Some(a),Some(b))=>a.eq_ignore_ascii_case(b),(None,None)=>true,_=>false };
                if same_hash && source == req.backup_source_id && target==req.target_device_id && t.kind == req.kind {
                let have = self.db.chunk_bitmap(&t.id)?;
                return Ok(CreateTransferResponse {
                    transfer_id: t.id,
                    chunk_size: t.chunk_size,
                    chunk_count: t.chunk_count,
                    have,
                    already_exists: false,
                    existing_file_id: None,
                    backup_item_id: None,
                });
                }
                self.db.update_transfer_status(&t.id,"aborted",Some("SOURCE_CHANGED"))?;
                if let Some(tmp)=&t.tmp_path { let _=std::fs::remove_file(tmp); }
            }
        }

        // Dedupe: same whole-file hash already in library → no upload (TR-09).
        if let Some(hash) = &req.root_hash {
            if let Some(file_id) = self.find_file_by_hash(hash)? {
                return Ok(CreateTransferResponse {
                    transfer_id: String::new(),
                    chunk_size,
                    chunk_count,
                    have: Default::default(),
                    already_exists: true,
                    existing_file_id: Some(file_id),
                    backup_item_id: None,
                });
            }
        }

        if self.db.list_transfers(Some(device_id),Some("open"))?.len() >= 8 { return Err(Error::RateLimited); }
        if self.db.list_transfers(None,Some("open"))?.len() >= 32 { return Err(Error::RateLimited); }

        // Preflight space (507 STORAGE_FULL).
        let free = fs2::free_space(&self.cfg.library_root)
            .map_err(|e| Error::StorageUnavailable(e.to_string()))?;
        if req.size > free.saturating_sub(256 * 1024 * 1024) {
            return Err(Error::StorageFull);
        }

        let transfer_id = ulid::Ulid::new().to_string();
        let tmp_path = self.cfg.tmp_dir().join(format!("{transfer_id}.part"));
        std::fs::create_dir_all(self.cfg.tmp_dir()).map_err(storage_io)?;

        // Preallocate the .part file (TRD §6.1).
        let f = OpenOptions::new().write(true).create_new(true).open(&tmp_path).map_err(storage_io)?;
        if let Err(error) = f.set_len(req.size).and_then(|_| f.sync_all()) {
            drop(f);
            let _ = std::fs::remove_file(&tmp_path);
            return Err(storage_io(error));
        }

        if let Err(error) = self.db.insert_transfer(
            &transfer_id,
            device_id,
            &req.kind,
            &name,
            req.size,
            req.mime.as_deref(),
            req.rel_path.as_deref(),
            chunk_size,
            chunk_count,
            req.root_hash.as_deref(),
            req.client_item_id.as_deref(),
            &tmp_path.to_string_lossy(),
            now_ms() + TRANSFER_IDLE_TTL_MS,
        ) { let _ = std::fs::remove_file(&tmp_path); return Err(error); }
        {
            let c = self.db.lock()?;
            c.execute("UPDATE transfers SET backup_source_id=?2,target_device_id=?3 WHERE id=?1",rusqlite::params![transfer_id,req.backup_source_id,req.target_device_id]).map_err(|e| Error::Db(e.to_string()))?;
        }

        Ok(CreateTransferResponse {
            transfer_id,
            chunk_size,
            chunk_count,
            have: Default::default(),
            already_exists: false,
            existing_file_id: None,
            backup_item_id: None,
        })
    }

    /// Verify and persist one chunk (API_SPEC §5 `PUT .../chunks/{n}`).
    /// Idempotent: re-PUT of a verified chunk with the same hash → Ok.
    pub fn put_chunk(&self, transfer_id: &str, idx: u64, header_hash: &str, bytes: &[u8]) -> Result<()> {
        let _guard = self.mutations.lock().map_err(|_| Error::Internal("transfer mutex poisoned".into()))?;
        validate_hash(header_hash)?;
        let t = self.must_get_open(transfer_id)?;
        self.check_activity(&t.device_id)?;
        if idx >= t.chunk_count {
            return Err(Error::BadRequest(format!("chunk index {idx} out of range")));
        }
        let expected = chunk_len(t.size, t.chunk_size, idx);
        if bytes.len() as u64 != expected {
            return Err(Error::BadRequest(format!(
                "chunk {idx} length {} != expected {expected}",
                bytes.len()
            )));
        }
        let actual = blake3::hash(bytes).to_hex().to_string();
        if !actual.eq_ignore_ascii_case(header_hash) {
            return Err(Error::HashMismatch { chunk: idx });
        }
        // Idempotent re-PUT (TRD §6.1).
        if let Some(h) = self.db.chunk_hash(transfer_id, idx)? {
            if h.eq_ignore_ascii_case(&actual) {
                return Ok(());
            }
        }

        let tmp = t.tmp_path.clone().ok_or_else(|| Error::Internal("transfer has no tmp path".into()))?;
        write_at(Path::new(&tmp), bytes, idx * t.chunk_size).map_err(storage_error)?;
        // SQLite must never advertise bytes that were not flushed to disk.
        File::options().write(true).open(&tmp)?.sync_data().map_err(storage_io)?;
        self.db.record_chunk(transfer_id, idx, &actual, bytes.len() as u64)?;
        Ok(())
    }

    pub fn status(&self, transfer_id: &str) -> Result<TransferStatus> {
        let t = self
            .db
            .get_transfer(transfer_id)?
            .ok_or(Error::TransferGone)?;
        let have = self.db.chunk_bitmap(transfer_id)?;
        Ok(t.to_status(have))
    }

    /// Finalize: verify whole-file BLAKE3 root, fsync, atomic rename, DB commit.
    pub fn complete(&self, transfer_id: &str, root_hash: &str) -> Result<CompleteResponse> {
        let _guard = self.mutations.lock().map_err(|_| Error::Internal("transfer mutex poisoned".into()))?;
        let transfer=self.db.get_transfer(transfer_id)?.ok_or(Error::TransferGone)?;
        self.check_activity(&transfer.device_id)?;
        self.complete_locked(transfer_id, root_hash)
    }

    fn complete_locked(&self, transfer_id: &str, root_hash: &str) -> Result<CompleteResponse> {
        validate_hash(root_hash)?;
        let t = self.db.get_transfer(transfer_id)?.ok_or(Error::TransferGone)?;
        // Startup recovery is local maintenance; normal routes are already
        // gated by certificate activity before entering this critical section.
        if t.status == "completed" {
            let c = self.db.lock()?;
            let result = c.query_row("SELECT id,hash,size,rel_path FROM files WHERE id=?1 AND deleted_at IS NULL",rusqlite::params![t.result_file_id],|r| Ok(CompleteResponse {file_id:r.get(0)?,hash:r.get(1)?,size:r.get::<_,i64>(2)? as u64,rel_path:r.get(3)?,verified:true,backup_item_id:None})).map_err(|e| Error::Db(e.to_string()))?;
            if !result.hash.eq_ignore_ascii_case(root_hash) { return Err(Error::RootHashMismatch); }
            return Ok(result);
        }
        if !["open","verifying"].contains(&t.status.as_str()) { return Err(Error::TransferGone); }
        if t.expected_root_hash.as_ref().is_some_and(|h| !h.eq_ignore_ascii_case(root_hash)) { return Err(Error::RootHashMismatch); }
        let have = self.db.chunk_bitmap(transfer_id)?;
        if have.count() != t.chunk_count { return Err(Error::Conflict("upload has missing chunks".into())); }
        let tmp = PathBuf::from(t.tmp_path.clone().ok_or_else(|| Error::Internal("no tmp path".into()))?);
        let journal: Option<(String,String,String,String,String,String)> = {
            use rusqlite::OptionalExtension;
            let c = self.db.lock()?;
            c.query_row("SELECT file_id,root_id,rel_path,name,category,hash FROM transfer_finalizations WHERE transfer_id=?1",[transfer_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(|e|Error::Db(e.to_string()))?
        };
        let (file_id,root_id,rel_path,name,category,computed) = match journal {
            Some(j) => j,
            None => {
                let file = OpenOptions::new().read(true).write(true).open(&tmp)?;
                file.sync_all().map_err(storage_io)?;
                let computed = hash_file(&file)?;
                if !computed.eq_ignore_ascii_case(root_hash) {
                    self.db.update_transfer_status(transfer_id,"open",Some("ROOT_HASH_MISMATCH"))?;
                    return Err(Error::RootHashMismatch);
                }
                let category = paths::category_for_mime(t.mime.as_deref()).to_owned();
                let dir_rel = match &t.rel_path {
                    Some(rp) => format!("{}/{}",paths::category_dir(&category,None),paths::sanitize_rel_path(rp)?),
                    None => paths::category_dir(&category,None),
                };
                let dir = paths::jail_join(&self.cfg.library_dir(),&dir_rel)?;
                std::fs::create_dir_all(&dir).map_err(storage_io)?;
                let name = paths::dedupe_name(&dir,&t.name);
                let rel_path = format!("{dir_rel}/{name}");
                let file_id = ulid::Ulid::new().to_string();
                let root_id = self.ensure_library_root()?;
                let mut c = self.db.lock()?;
                let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|Error::Db(e.to_string()))?;
                tx.execute("INSERT INTO transfer_finalizations(transfer_id,file_id,root_id,rel_path,name,category,hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",rusqlite::params![transfer_id,file_id,root_id,rel_path,name,category,computed,now_ms()]).map_err(|e|Error::Db(e.to_string()))?;
                tx.execute("UPDATE transfers SET status='verifying',updated_at=?2 WHERE id=?1",rusqlite::params![transfer_id,now_ms()]).map_err(|e|Error::Db(e.to_string()))?;
                tx.commit().map_err(|e|Error::Db(e.to_string()))?;
                (file_id,root_id,rel_path,name,category,computed)
            }
        };
        if !computed.eq_ignore_ascii_case(root_hash) { return Err(Error::RootHashMismatch); }
        let dest = paths::jail_join(&self.cfg.library_dir(),&rel_path)?;
        if tmp.exists() {
            if dest.exists() { return Err(Error::Conflict("finalization destination exists".into())); }
            #[cfg(windows)] rename_write_through(&tmp, &dest).map_err(storage_error)?;
            #[cfg(not(windows))] {std::fs::hard_link(&tmp,&dest).map_err(storage_io)?;std::fs::remove_file(&tmp)?;}
            #[cfg(unix)]
            if let Some(parent) = dest.parent() { File::open(parent)?.sync_all().map_err(storage_io)?; }
        }
        let file = File::open(&dest)?;
        if file.metadata()?.len() != t.size || !hash_file(&file)?.eq_ignore_ascii_case(&computed) { return Err(Error::RootHashMismatch); }
        let chunk_hashes = self.all_chunk_hashes(transfer_id)?;
        self.commit_file_rows(&file_id,&root_id,&rel_path,&name,&category,&t,&computed,&chunk_hashes)?;
        Ok(CompleteResponse { file_id,verified:true,hash:computed,size:t.size,rel_path,backup_item_id:None })
    }

    pub fn abort(&self, transfer_id: &str) -> Result<()> {
        let _guard = self.mutations.lock().map_err(|_| Error::Internal("transfer mutex poisoned".into()))?;
        let t = self
            .db
            .get_transfer(transfer_id)?
            .ok_or(Error::TransferGone)?;
        if t.status == "completed" { return Err(Error::Conflict("completed transfer cannot be aborted".into())); }
        if t.status == "verifying" { return Err(Error::Conflict("finalization in progress; retry completion".into())); }
        if let Some(tmp) = &t.tmp_path {
            let _ = std::fs::remove_file(tmp);
        }
        self.db.update_transfer_status(transfer_id, "aborted", None)?;
        Ok(())
    }

    /// Startup reconciliation (AGENTS.md §6): expire idle sessions, delete
    /// orphan .part files older than the TTL.
    pub fn reconcile_on_startup(&self) -> Result<()> {
        let _guard = self.mutations.lock().map_err(|_| Error::Internal("transfer mutex poisoned".into()))?;
        for t in self.db.list_transfers(None,Some("verifying"))? {
            let hash: String = { let c = self.db.lock()?; c.query_row("SELECT hash FROM transfer_finalizations WHERE transfer_id=?1",[&t.id],|r|r.get(0)).map_err(|e|Error::Db(e.to_string()))? };
            // A transient disk or database failure must not prevent the Hub
            // from starting. Leave the journal and partial file for retry.
            if let Err(error) = self.complete_locked(&t.id,&hash) {
                tracing::warn!(transfer_id = %t.id, %error, "transfer finalization remains pending");
            }
        }
        for t in self.db.list_transfers(None,Some("open"))? {
            let chunks = self.all_chunk_hashes(&t.id)?;
            let path = t.tmp_path.as_deref().ok_or_else(||Error::Internal("missing tmp path".into()))?;
            let mut file = match File::open(path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => { self.db.update_transfer_status(&t.id,"failed",Some("PART_MISSING"))?; continue; },
                Err(e) => return Err(e.into()),
            };
            use std::io::{Read,Seek,SeekFrom};
            for (idx,hash) in chunks {
                let mut bytes = vec![0; chunk_len(t.size,t.chunk_size,idx) as usize];
                let valid = file.seek(SeekFrom::Start(idx*t.chunk_size)).and_then(|_|file.read_exact(&mut bytes)).is_ok() && blake3::hash(&bytes).to_hex().as_str().eq_ignore_ascii_case(&hash);
                if !valid { let c = self.db.lock()?; c.execute("DELETE FROM transfer_chunks WHERE transfer_id=?1 AND idx=?2",rusqlite::params![t.id,idx as i64]).map_err(|e|Error::Db(e.to_string()))?; }
            }
            let c = self.db.lock()?;
            c.execute("UPDATE transfers SET bytes_verified=(SELECT COALESCE(SUM(size),0) FROM transfer_chunks WHERE transfer_id=?1) WHERE id=?1",[&t.id]).map_err(|e|Error::Db(e.to_string()))?;
        }
        for path in self.db.expire_idle_transfers(TRANSFER_IDLE_TTL_MS)? {
            let _ = std::fs::remove_file(path);
        }
        let mut retained = self.db.list_transfers(None, Some("open"))?;
        retained.extend(self.db.list_transfers(None, Some("verifying"))?);
        let known: std::collections::HashSet<String> = retained.iter()
            .filter_map(|t| t.tmp_path.clone())
            .collect();
        let cutoff = now_ms() - TRANSFER_IDLE_TTL_MS;
        for entry in std::fs::read_dir(self.cfg.tmp_dir())?.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "part").unwrap_or(false)
                && !known.contains(&p.to_string_lossy().to_string())
            {
                let mtime_old = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| (d.as_millis() as i64) < cutoff)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if mtime_old {
                    tracing::info!("deleting expired orphan partial upload");
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        Ok(())
    }

    // ---- helpers ----

    fn must_get_open(&self, id: &str) -> Result<TransferRow> {
        let t = self.db.get_transfer(id)?.ok_or(Error::TransferGone)?;
        if t.status != "open" {
            return Err(Error::TransferGone);
        }
        Ok(t)
    }

    fn check_activity(&self,device_id:&str)->Result<()> {
        if self.db.get_setting("sharing.paused")?.as_deref()==Some("true"){return Err(Error::StorageUnavailable("sharing paused".into()));}
        let device=self.db.device_by_id(device_id)?.ok_or(Error::Unauthenticated)?;
        if device.status!="active" {return Err(Error::DeviceRevoked);}
        if !device.has_scope("transfer"){return Err(Error::ForbiddenScope("transfer".into()));}Ok(())
    }

    fn find_file_by_hash(&self,hash:&str)->Result<Option<String>> {
        let c=self.db.lock()?;
        let mut st=c.prepare("SELECT f.id,r.path,f.rel_path,f.size FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE f.hash=?1 AND f.deleted_at IS NULL AND r.is_active=1").map_err(|e|Error::Db(e.to_string()))?;
        let rows=st.query_map([hash],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))).map_err(|e|Error::Db(e.to_string()))?;
        for row in rows {let (id,root,rel,size)=row.map_err(|e|Error::Db(e.to_string()))?;
            let valid=(||->Result<bool>{let path=paths::jail_join(Path::new(&root),&rel)?;let f=File::open(path)?;Ok(f.metadata()?.len()==size as u64 && hash_file(&f)?.eq_ignore_ascii_case(hash))})();
            if matches!(valid,Ok(true)){return Ok(Some(id));}
        }
        Ok(None)
    }

    fn ensure_library_root(&self) -> Result<String> {
        let c = self.db.lock()?;
        let existing: Option<String> = c
            .query_row(
                "SELECT id FROM storage_roots WHERE kind='library' AND is_active=1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = ulid::Ulid::new().to_string();
        c.execute(
            "INSERT INTO storage_roots (id, kind, path, label, is_active, created_at)
             VALUES (?1,'library',?2,'Library',1,?3)",
            rusqlite::params![id, self.cfg.library_dir().to_string_lossy().to_string(), now_ms()],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
        Ok(id)
    }

    fn all_chunk_hashes(&self, transfer_id: &str) -> Result<Vec<(u64, String)>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT idx, hash FROM transfer_chunks WHERE transfer_id=?1 ORDER BY idx")
            .map_err(|e| Error::Db(e.to_string()))?;
        let rows = st
            .query_map([transfer_id], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?)))
            .map_err(|e| Error::Db(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Db(e.to_string()))?;
        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_file_rows(
        &self,
        file_id: &str,
        root_id: &str,
        rel_path: &str,
        name: &str,
        category: &str,
        t: &TransferRow,
        hash: &str,
        chunk_hashes: &[(u64, String)],
    ) -> Result<()> {
        let mut c = self.db.lock()?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| Error::Db(e.to_string()))?;
        tx.execute(
            "INSERT INTO files (id, root_id, rel_path, name, category, mime, size, hash,
                chunk_size, source_mode, origin_device_id, client_item_id, created_at, last_verified_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'upload',?10,?11,?12,?12)",
            rusqlite::params![
                file_id, root_id, rel_path, name, category, t.mime, t.size as i64, hash,
                t.chunk_size as i64, t.device_id, t.client_item_id, now_ms()
            ],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
        let mut st = tx
            .prepare("INSERT INTO file_chunks (file_id, idx, hash) VALUES (?1,?2,?3)")
            .map_err(|e| Error::Db(e.to_string()))?;
        for (idx, h) in chunk_hashes {
            st.execute(rusqlite::params![file_id, *idx as i64, h])
                .map_err(|e| Error::Db(e.to_string()))?;
        }
        drop(st);
        tx.execute("UPDATE transfers SET result_file_id=?2,status='completed',error_code=NULL,completed_at=?3,updated_at=?3 WHERE id=?1",rusqlite::params![t.id,file_id,now_ms()]).map_err(|e|Error::Db(e.to_string()))?;
        tx.execute("DELETE FROM transfer_finalizations WHERE transfer_id=?1",[&t.id]).map_err(|e|Error::Db(e.to_string()))?;
        tx.commit().map_err(|e| Error::Db(e.to_string()))?;
        Ok(())
    }
}

fn validate_hash(hash: &str) -> Result<()> {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(Error::BadRequest("expected 64 hexadecimal hash characters".into())); }
    Ok(())
}

fn storage_io(error: std::io::Error) -> Error {
    #[cfg(unix)]
    if error.raw_os_error() == Some(28) { return Error::StorageFull; }
    #[cfg(windows)]
    if error.raw_os_error() == Some(112) { return Error::StorageFull; }
    Error::Io(error)
}

fn storage_error(error: Error) -> Error {
    match error { Error::Io(io) => storage_io(io), other => other }
}

fn chunk_len(file_size: u64, chunk_size: u64, idx: u64) -> u64 {
    let start = idx * chunk_size;
    file_size.saturating_sub(start).min(chunk_size)
}

/// Stream-hash an open file with BLAKE3 (off the async reactor — call from
/// spawn_blocking or a sync context).
pub fn hash_file(mut file: &File) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(unix)]
fn write_at(path: &Path, bytes: &[u8], offset: u64) -> Result<()> {
    use std::os::unix::fs::FileExt;
    let f = OpenOptions::new().write(true).open(path)?;
    f.write_all_at(bytes, offset)?;
    Ok(())
}

#[cfg(windows)]
fn rename_write_through(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // No REPLACE_EXISTING: a colliding library file must never be overwritten.
    let moved = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if moved == 0 { return Err(std::io::Error::last_os_error().into()); }
    Ok(())
}

#[cfg(windows)]
fn write_at(path: &Path, bytes: &[u8], offset: u64) -> Result<()> {
    use std::os::windows::fs::FileExt;
    let f = OpenOptions::new().write(true).open(path)?;
    let mut written = 0;
    while written < bytes.len() {
        let n = f.seek_write(&bytes[written..],offset + written as u64)?;
        if n == 0 { return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()); }
        written += n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hh_db::DeviceRow;

    fn setup() -> (TransferEngine, Db, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config {
            hub_name: "Test".into(),
            data_dir: tmp.path().join("data"),
            library_root: tmp.path().join("lib"),
            log_dir: tmp.path().join("logs"),
            second_copy_root: None,
            features: Default::default(),
        };
        cfg.ensure_dirs().unwrap();
        let db = Db::open(&cfg.db_path()).unwrap();
        db.insert_device(
            &DeviceRow {
                id: "d1".into(), name: "n".into(), platform: "android".into(),
                model: None, app_version: None, cert_serial: "s".into(),
                cert_expires_at: 0, scopes: vec!["transfer".into()], paired_at: 0, last_seen_at: None,
                status: "active".into(),
            },
            "PEM", "PUB",
        )
        .unwrap();
        (TransferEngine::new(db.clone(), cfg), db, tmp)
    }

    fn blake3_hex(b: &[u8]) -> String {
        blake3::hash(b).to_hex().to_string()
    }

    #[test]
    fn upload_chunk_complete_resume() {
        let (eng, _db, _tmp) = setup();
        let data = vec![0xABu8; (DEFAULT_CHUNK_SIZE + 123) as usize]; // 2 chunks
        let root = blake3_hex(&data);
        let req = CreateTransferRequest {
            name: "big.bin".into(),
            size: data.len() as u64,
            mime: Some("application/octet-stream".into()),
            kind: "send".into(),
            rel_path: None,
            chunk_size: Some(DEFAULT_CHUNK_SIZE),
            client_item_id: Some("item-1".into()),
            root_hash: Some(root.clone()),
            taken_at: None,
            target_device_id: None, backup_source_id: None,
        };
        let r = eng.create("d1", &req).unwrap();
        assert_eq!(r.chunk_count, 2);
        assert!(!r.already_exists);

        let c0 = &data[..DEFAULT_CHUNK_SIZE as usize];
        let c1 = &data[DEFAULT_CHUNK_SIZE as usize..];
        eng.put_chunk(&r.transfer_id, 0, &blake3_hex(c0), c0).unwrap();

        // Simulate disconnect: resume via client_item_id returns have=[[0,0]].
        let r2 = eng.create("d1", &req).unwrap();
        assert_eq!(r2.transfer_id, r.transfer_id);
        assert_eq!(r2.have.ranges, vec![(0, 0)]);

        eng.put_chunk(&r.transfer_id, 1, &blake3_hex(c1), c1).unwrap();
        let done = eng.complete(&r.transfer_id, &root).unwrap();
        assert!(done.verified);
        assert!(done.rel_path.starts_with("Downloads/"));

        // Dedupe: sending same hash again short-circuits (TR-09).
        let r3 = eng.create("d1", &req).unwrap();
        assert!(r3.already_exists);
        assert_eq!(r3.existing_file_id.as_deref(), Some(done.file_id.as_str()));
    }

    #[test]
    fn hash_mismatch_rejected() {
        let (eng, _db, _tmp) = setup();
        let data = vec![7u8; 100];
        let req = CreateTransferRequest {
            name: "x.bin".into(), size: 100, mime: None, kind: "send".into(),
            rel_path: None, chunk_size: None, client_item_id: None,
            root_hash: Some(blake3_hex(&data)), taken_at: None, target_device_id: None, backup_source_id: None,
        };
        let r = eng.create("d1", &req).unwrap();
        let err = eng.put_chunk(&r.transfer_id, 0, &blake3_hex(b"wrong"), &data).unwrap_err();
        assert!(matches!(err, Error::HashMismatch { chunk: 0 }));
    }

    #[test]
    fn root_mismatch_is_422() {
        let (eng, _db, _tmp) = setup();
        let data = vec![1u8; 50];
        let req = CreateTransferRequest {
            name: "y.bin".into(), size: 50, mime: None, kind: "send".into(),
            rel_path: None, chunk_size: None, client_item_id: None,
            root_hash: None, taken_at: None, target_device_id: None, backup_source_id: None,
        };
        let r = eng.create("d1", &req).unwrap();
        eng.put_chunk(&r.transfer_id, 0, &blake3_hex(&data), &data).unwrap();
        assert!(matches!(
            eng.complete(&r.transfer_id, &blake3_hex(b"other")),
            Err(Error::RootHashMismatch)
        ));
    }
}
