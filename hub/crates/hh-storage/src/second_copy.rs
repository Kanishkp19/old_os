//! Second-copy backup to an external drive (TRD §7.5, FR-4.4):
//! incremental mirror by hash, verified after copy, resumable (ST-07).

use hh_core::error::Result;
use hh_core::time::now_ms;
use rusqlite::params;

use crate::{db_e, StorageService};

impl StorageService {
    /// Run one second-copy pass. Returns (copied, failed, run_id).
    pub fn run_second_copy(&self) -> Result<(u64, u64, String)> {
        let target = self
            .cfg
            .second_copy_root
            .clone()
            .ok_or_else(|| hh_core::Error::BadRequest("no second-copy target configured".into()))?;
        std::fs::create_dir_all(&target)?;

        let run_id = ulid::Ulid::new().to_string();
        let target_root_id = ulid::Ulid::new().to_string();
        {
            let c = self.db.lock()?;
            c.execute(
                "INSERT OR IGNORE INTO storage_roots (id, kind, path, label, is_active, created_at)
                 VALUES (?1,'second_copy',?2,'Second copy',1,?3)",
                params![target_root_id, target.to_string_lossy().to_string(), now_ms()],
            )
            .map_err(db_e)?;
            c.execute(
                "INSERT INTO second_copy_runs (id, target_root_id, started_at, status)
                 VALUES (?1,?2,?3,'running')",
                params![run_id, target_root_id, now_ms()],
            )
            .map_err(db_e)?;
        }

        let files: Vec<(String, String, String, i64)> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare("SELECT id, rel_path, hash, size FROM files WHERE deleted_at IS NULL")
                .map_err(db_e)?;
            let rows = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<(String, String, String, i64)>, _>>()
                .map_err(db_e)?;
            rows
        };

        let mut copied = 0u64;
        let mut failed = 0u64;
        let mut bytes = 0u64;
        for (id, rel, hash, size) in files {
            let src = hh_core::paths::jail_join(&self.cfg.library_dir(), &rel)?;
            let dst = target.join(&rel);
            // Incremental: skip if destination already matches the hash.
            if dst.exists() {
                if let Ok(f) = std::fs::File::open(&dst) {
                    if let Ok(h) = hh_transfer::hash_file(&f) {
                        if h.eq_ignore_ascii_case(&hash) {
                            continue;
                        }
                    }
                }
            }
            let ok = (|| -> Result<()> {
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p)?;
                }
                // Copy via temp file on the same volume, verify, rename.
                let tmp = dst.with_extension("hh-copy");
                std::fs::copy(&src, &tmp)?;
                let f = std::fs::File::open(&tmp)?;
                f.sync_all()?;
                let h = hh_transfer::hash_file(&f)?;
                if !h.eq_ignore_ascii_case(&hash) {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(hh_core::Error::RootHashMismatch);
                }
                std::fs::rename(&tmp, &dst)?;
                Ok(())
            })();
            match ok {
                Ok(()) => {
                    copied += 1;
                    bytes += size as u64;
                }
                Err(e) => {
                    failed += 1;
                    tracing::warn!(file_id = %id, error = %e, "second-copy failed for file");
                }
            }
        }

        let status = if failed == 0 { "ok" } else { "partial" };
        let c = self.db.lock()?;
        c.execute(
            "UPDATE second_copy_runs SET finished_at=?2, files_copied=?3, bytes_copied=?4, files_failed=?5, status=?6
             WHERE id=?1",
            params![run_id, now_ms(), copied as i64, bytes as i64, failed as i64, status],
        )
        .map_err(db_e)?;
        Ok((copied, failed, run_id))
    }

    /// Age of last successful second copy in ms (None = never) — drives the
    /// "1 copy only" nudge (UI_UX §4.7).
    pub fn last_second_copy_age_ms(&self) -> Result<Option<i64>> {
        let c = self.db.lock()?;
        let last: Option<i64> = c
            .query_row(
                "SELECT MAX(finished_at) FROM second_copy_runs WHERE status IN ('ok','partial')",
                [],
                |r| r.get(0),
            )
            .map_err(db_e)?;
        Ok(last.map(|t| now_ms() - t))
    }
}
