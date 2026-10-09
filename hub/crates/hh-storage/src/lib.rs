//! hh-storage: library browsing, trash, exact dedupe, integrity scrub,
//! SMART health snapshots, second copy (TRD §7, BACKEND_SCHEMA §2/§5/§6).
//!
//! Hard rules honored here:
//! - Partial files are never visible (only `files` rows, written at finalize).
//! - Nothing is auto-deleted: trash with 30-day retention, user-approved only.
//! - Existing Windows data is never modified (FR-1.4).

pub mod dedupe;
pub mod health;
pub mod jobs;
pub mod library;
pub mod perceptual;
pub mod scrub;
pub mod second_copy;
pub mod trash;

use hh_core::error::Result;
use hh_core::Config;
use hh_db::Db;
use serde::Serialize;

/// rusqlite → hub error mapping shared by every module in this crate.
pub(crate) fn db_e(e: rusqlite::Error) -> hh_core::Error {
    hh_core::Error::Db(e.to_string())
}

#[derive(Clone)]
pub struct StorageService {
    pub db: Db,
    pub cfg: Config,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorySummary {
    pub category: String,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySummary {
    pub categories: Vec<CategorySummary>,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub copies: u8, // 1 = library only, 2 = second copy configured
    pub reclaimable_trash_bytes: u64,
    pub missing_files: u64,
}

impl StorageService {
    pub fn new(db: Db, cfg: Config) -> Self {
        Self { db, cfg }
    }

    pub fn library_summary(&self) -> Result<LibrarySummary> {
        let rows:Vec<(String,String,String,bool,bool)>={
            let c=self.db.lock()?;
            let mut st=c.prepare("SELECT f.category,r.path,f.rel_path,f.deleted_at IS NOT NULL,f.source_mode='keep_in_place' FROM files f JOIN storage_roots r ON r.id=f.root_id WHERE r.is_active=1")
                .map_err(|e|hh_core::Error::Db(e.to_string()))?;
            let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
                .map_err(|e|hh_core::Error::Db(e.to_string()))?
                .collect::<std::result::Result<_,_>>().map_err(|e|hh_core::Error::Db(e.to_string()))?;
            rows
        };
        let mut totals:std::collections::BTreeMap<String,(u64,u64)>=std::collections::BTreeMap::new();
        let mut reclaimable_trash_bytes=0u64;let mut missing_files=0u64;
        for (category,root,rel,deleted,external) in rows {
            let path=match hh_core::paths::jail_join(std::path::Path::new(&root),&rel) {
                Ok(path)=>path,
                Err(hh_core::Error::Io(e)) if e.kind()==std::io::ErrorKind::NotFound=>{missing_files+=1;continue;},
                Err(error)=>return Err(error),
            };
            let size=match std::fs::metadata(&path) {
                Ok(meta) if meta.is_file()=>meta.len(),
                Ok(_)=>{missing_files+=1;continue;},
                Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{missing_files+=1;continue;},
                Err(e)=>return Err(e.into()),
            };
            if deleted {
                if !external {reclaimable_trash_bytes=reclaimable_trash_bytes.saturating_add(size);}
            } else {
                let entry=totals.entry(category).or_default();entry.0+=1;entry.1=entry.1.saturating_add(size);
            }
        }
        let categories=totals.into_iter().map(|(category,(count,bytes))|CategorySummary{category,count,bytes}).collect();

        let free = fs2::free_space(&self.cfg.library_root).unwrap_or(0);
        let total = fs2::total_space(&self.cfg.library_root).unwrap_or(0);
        let coverage=self.copy_coverage()?;
        let copies=if coverage["all_protected"].as_bool()==Some(true){2}else{1};
        Ok(LibrarySummary { categories, free_bytes: free, total_bytes: total, copies, reclaimable_trash_bytes, missing_files })
    }

    /// Free-space alert thresholds (TRD §7.4): warn 15%, critical 5%.
    pub fn check_free_space_alerts(&self) -> Result<()> {
        let (free,total)=match (fs2::free_space(&self.cfg.library_root),fs2::total_space(&self.cfg.library_root)) {
            (Ok(free),Ok(total)) if total>0=>(free,total),
            _=>{
                self.db.set_condition_alert(Some("critical"),"STORAGE_UNAVAILABLE","The Home library drive is unavailable. Check its connection before storing more files.")?;
                self.db.set_condition_alert(None,"LOW_SPACE","")?;
                return Ok(());
            }
        };
        self.db.set_condition_alert(None,"STORAGE_UNAVAILABLE","")?;
        let pct = (free as f64 / total.max(1) as f64) * 100.0;
        if pct < 5.0 {
            self.db.set_condition_alert(
                Some("critical"),
                "LOW_SPACE",
                &format!("Home is almost full ({pct:.0}% free). Free up space or add a drive."),
            )?;
        } else if pct < 15.0 {
            self.db.set_condition_alert(
                Some("warning"),
                "LOW_SPACE",
                &format!("Home is getting full ({pct:.0}% free)."),
            )?;
        } else {self.db.set_condition_alert(None,"LOW_SPACE","")?;}
        let target=self.db.get_setting("second_copy.root")?.filter(|v|!v.is_empty()).map(std::path::PathBuf::from).or_else(||self.cfg.second_copy_root.clone());
        let coverage=self.copy_coverage()?;
        let disconnected=target.is_some() && coverage["drive_connected"].as_bool()!=Some(true);
        self.db.set_condition_alert(disconnected.then_some("warning"),"SECOND_COPY_DISCONNECTED",
            "The second-copy drive is disconnected. Reconnect it to protect new and changed files.")?;
        let exposed=coverage["total_files"].as_i64().unwrap_or(0)>0 && coverage["all_protected"].as_bool()!=Some(true);
        self.db.set_condition_alert(exposed.then_some("warning"),"SINGLE_COPY",
            "Some files have only one verified copy. Connect a second drive and run protection.")?;
        Ok(())
    }
}

/// Publish without overwriting another file. On Windows rename fails if the
/// destination exists; on Unix a same-volume hard link provides exclusivity.
pub(crate) fn publish_file(src:&std::path::Path,dst:&std::path::Path)->Result<()> {
    #[cfg(windows)] {std::fs::rename(src,dst)?;}
    #[cfg(not(windows))] {std::fs::hard_link(src,dst)?;std::fs::remove_file(src)?;}
    Ok(())
}

pub(crate) fn verified_copy(src:&std::path::Path,dst:&std::path::Path,expected:&str)->Result<()> {
    if let Some(p)=dst.parent(){std::fs::create_dir_all(p)?;}
    let tmp=dst.with_file_name(format!(".hh-copy-{}",ulid::Ulid::new()));
    let result=(||->Result<()> {
        let mut input=std::fs::File::open(src)?;
        let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        std::io::copy(&mut input,&mut output)?;output.sync_all()?;drop(output);
        let f=std::fs::File::open(&tmp)?;
        if !hh_transfer::hash_file(&f)?.eq_ignore_ascii_case(expected) {return Err(hh_core::Error::RootHashMismatch);}
        drop(f);publish_file(&tmp,dst)?;Ok(())
    })();
    if result.is_err(){let _=std::fs::remove_file(tmp);}
    result
}

/// Keep a damaged destination until a replacement has been fully verified.
pub(crate) fn verified_replace(src:&std::path::Path,dst:&std::path::Path,expected:&str)->Result<()> {
    let repair=dst.with_file_name(format!(".hh-repair-{}",ulid::Ulid::new()));
    verified_copy(src,&repair,expected)?;
    let result=atomic_replace(&repair,dst);
    // If Windows failed after moving the old name, the verified repair may
    // be the only intact copy. Leave it available when the destination is gone.
    if result.is_err()&&dst.exists(){let _=std::fs::remove_file(&repair);}
    result
}

/// Replace only with a fully verified same-volume temporary file.
pub(crate) fn atomic_replace(src:&std::path::Path,dst:&std::path::Path)->Result<()> {
    #[cfg(not(windows))] {std::fs::rename(src,dst)?;}
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        #[link(name="kernel32")] extern "system" {fn ReplaceFileW(replaced:*const u16,replacement:*const u16,backup:*const u16,flags:u32,exclude:*mut std::ffi::c_void,reserved:*mut std::ffi::c_void)->i32;}
        let backup=dst.with_file_name(format!(".hh-replaced-{}",ulid::Ulid::new()));
        let a:Vec<u16>=dst.as_os_str().encode_wide().chain(Some(0)).collect();
        let b:Vec<u16>=src.as_os_str().encode_wide().chain(Some(0)).collect();
        let old:Vec<u16>=backup.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe{ReplaceFileW(a.as_ptr(),b.as_ptr(),old.as_ptr(),0,std::ptr::null_mut(),std::ptr::null_mut())}==0 {
            let error=std::io::Error::last_os_error();
            // ERROR_UNABLE_TO_MOVE_REPLACEMENT_2 may move the original to
            // the backup path even though replacement failed.
            if !dst.exists()&&backup.exists(){let _=std::fs::rename(&backup,dst);}
            return Err(error.into());
        }
        let _=std::fs::remove_file(backup);
    }
    Ok(())
}

#[cfg(test)]
mod verified_replace_tests {
    #[test]
    fn repairs_only_from_a_verified_source() {
        let dir=std::env::temp_dir().join(format!("hh-copy-test-{}",ulid::Ulid::new()));
        std::fs::create_dir(&dir).unwrap();
        let src=dir.join("source");let dst=dir.join("copy");
        std::fs::write(&src,b"correct bytes").unwrap();
        std::fs::write(&dst,b"damaged bytes").unwrap();
        let expected=blake3::hash(b"correct bytes").to_hex().to_string();
        super::verified_replace(&src,&dst,&expected).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(),b"correct bytes");
        std::fs::write(&src,b"changed source").unwrap();
        assert!(super::verified_replace(&src,&dst,&expected).is_err());
        assert_eq!(std::fs::read(&dst).unwrap(),b"correct bytes");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
