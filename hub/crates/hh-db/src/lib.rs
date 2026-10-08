//! hh-db: SQLite (WAL) access with forward-only migrations.
//!
//! Conventions (AGENTS.md §4): one writer connection guarded by a mutex,
//! `BEGIN IMMEDIATE` for multi-statement writes, migrations live as
//! numbered SQL files in `migrations/`, forward-only, with a pre-migration
//! backup (`hub.db.bak-<version>`, keep last 3 — BACKEND_SCHEMA §10).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use rusqlite::{params, Connection, OptionalExtension};

mod queries;
pub use queries::*;

/// Embedded migration files, applied in filename order.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_core.sql"),
    include_str!("../migrations/0002_storage.sql"),
    include_str!("../migrations/0003_transfers.sql"),
    include_str!("../migrations/0004_photos.sql"),
    include_str!("../migrations/0005_health.sql"),
    include_str!("../migrations/0006_system.sql"),
    include_str!("../migrations/0007_reliability.sql"),
    include_str!("../migrations/0008_storage_safety.sql"),
    include_str!("../migrations/0009_delivery.sql"),
    include_str!("../migrations/0010_renewal.sql"),
    include_str!("../migrations/0011_cleanup_recovery.sql"),
];

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl Db {
    /// Open (creating if needed) and migrate to the latest schema.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(db_err)?;
        let db = Self { conn: Arc::new(Mutex::new(conn)), path: path.to_path_buf() };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    /// Consistent online snapshot without running migrations on the source.
    pub fn snapshot(source:&Path,destination:&Path)->Result<()> {
        if destination.exists(){return Err(Error::Conflict("backup destination exists".into()));}
        let c=Connection::open_with_flags(source,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(db_err)?;
        c.busy_timeout(std::time::Duration::from_secs(30)).map_err(db_err)?;
        c.execute("VACUUM INTO ?1",params![destination.to_string_lossy().as_ref()]).map_err(db_err)?;
        std::fs::File::open(destination)?.sync_all()?;Ok(())
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        let db = Self { conn: Arc::new(Mutex::new(conn)), path: PathBuf::from(":memory:") };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    fn configure(&self) -> Result<()> {
        let c = self.lock()?;
        // BACKEND_SCHEMA header pragmas: integrity over speed.
        c.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(db_err)
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| Error::Db("connection mutex poisoned".into()))
    }

    fn schema_version(&self) -> Result<i64> {
        let c = self.lock()?;
        let v: Option<i64> = c
            .query_row("SELECT MAX(schema_version) FROM hub", [], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .flatten();
        Ok(v.unwrap_or(0))
    }

    fn migrate(&self) -> Result<()> {
        // Refuse to run against a DB newer than this binary (BACKEND_SCHEMA §10).
        let current = {
            let c = self.lock()?;
            let has_hub: bool = c
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hub'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(db_err)?
                > 0;
            drop(c);
            if has_hub { self.schema_version()? } else { 0 }
        };

        let target = MIGRATIONS.len() as i64;
        if current > target {
            return Err(Error::Db(format!(
                "database schema v{current} is newer than this binary supports (v{target}); update Home Hub"
            )));
        }

        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let version = (i + 1) as i64;
            if version <= current {
                continue;
            }
            self.backup_before_migration(version - 1)?;
            let mut c = self.lock()?;
            let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            if current == 0 && version == 1 {
                // First boot: single hub identity row. CA fields are filled by
                // hh-auth during provisioning; placeholder until then.
                tx.execute(
                    "INSERT INTO hub (id, name, created_at, ca_cert_pem, ca_key_ref, server_cert_pem, schema_version)
                     VALUES (?1, 'Home Hub', ?2, '', '', '', ?3)",
                    params![ulid::Ulid::new().to_string(), now_ms(), version],
                )
                .map_err(db_err)?;
            } else {
                tx.execute("UPDATE hub SET schema_version = ?1", params![version])
                    .map_err(db_err)?;
            }
            tx.commit().map_err(db_err)?;
            tracing::info!(version, "applied migration");
        }
        Ok(())
    }

    /// Copy hub.db → hub.db.bak-<version>; keep the last 3 (BACKEND_SCHEMA §10).
    fn backup_before_migration(&self, from_version: i64) -> Result<()> {
        if self.path.as_os_str() == ":memory:" || from_version == 0 {
            return Ok(());
        }
        let bak = self.path.with_extension(format!("db.bak-{from_version}-{}",ulid::Ulid::new()));
        let c = self.lock()?;
        // SQLite takes a consistent snapshot including committed WAL pages.
        c.execute("VACUUM INTO ?1",params![bak.to_string_lossy().as_ref()]).map_err(db_err)?;
        std::fs::File::open(&bak)?.sync_all()?;
        drop(c);
        // prune older backups, keep last 3
        let mut backups: Vec<PathBuf> = std::fs::read_dir(self.path.parent().unwrap_or(Path::new(".")))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().map(|n| n.to_string_lossy().contains(".bak-")).unwrap_or(false))
            .collect();
        backups.sort_by_key(|p|p.metadata().and_then(|m|m.modified()).ok());
        while backups.len() > 3 {
            let old = backups.remove(0);
            let _ = std::fs::remove_file(old);
        }
        Ok(())
    }
}

pub(crate) fn db_err(e: rusqlite::Error) -> Error {
    Error::Db(e.to_string())
}

pub(crate) fn opt<T>(r: std::result::Result<T, rusqlite::Error>) -> Result<Option<T>> {
    r.optional().map_err(db_err)
}
