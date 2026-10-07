//! hh-storage: library browsing, trash, exact dedupe, integrity scrub,
//! SMART health snapshots, second copy (TRD §7, BACKEND_SCHEMA §2/§5/§6).
//!
//! Hard rules honored here:
//! - Partial files are never visible (only `files` rows, written at finalize).
//! - Nothing is auto-deleted: trash with 30-day retention, user-approved only.
//! - Existing Windows data is never modified (FR-1.4).

pub mod dedupe;
pub mod health;
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
}

impl StorageService {
    pub fn new(db: Db, cfg: Config) -> Self {
        Self { db, cfg }
    }

    pub fn library_summary(&self) -> Result<LibrarySummary> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare(
                "SELECT category, COUNT(*), COALESCE(SUM(size),0) FROM files
                 WHERE deleted_at IS NULL GROUP BY category",
            )
            .map_err(|e| hh_core::Error::Db(e.to_string()))?;
        let categories = st
            .query_map([], |r| {
                Ok(CategorySummary {
                    category: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u64,
                    bytes: r.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(|e| hh_core::Error::Db(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| hh_core::Error::Db(e.to_string()))?;
        drop(st);
        drop(c);

        let free = fs2::free_space(&self.cfg.library_root).unwrap_or(0);
        let total = fs2::total_space(&self.cfg.library_root).unwrap_or(0);
        let copies = if self.cfg.second_copy_root.is_some() { 2 } else { 1 };
        Ok(LibrarySummary { categories, free_bytes: free, total_bytes: total, copies })
    }

    /// Free-space alert thresholds (TRD §7.4): warn 15%, critical 5%.
    pub fn check_free_space_alerts(&self) -> Result<()> {
        let free = fs2::free_space(&self.cfg.library_root).unwrap_or(0);
        let total = fs2::total_space(&self.cfg.library_root).unwrap_or(1);
        let pct = (free as f64 / total.max(1) as f64) * 100.0;
        if pct < 5.0 {
            self.db.create_alert(
                "critical",
                "LOW_SPACE",
                &format!("Home is almost full ({pct:.0}% free). Free up space or add a drive."),
            )?;
        } else if pct < 15.0 {
            self.db.create_alert(
                "warning",
                "LOW_SPACE",
                &format!("Home is getting full ({pct:.0}% free)."),
            )?;
        }
        Ok(())
    }
}
