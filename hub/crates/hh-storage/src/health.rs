//! SMART/disk health collector (TRD §7.4, FR-6.1): periodic snapshots,
//! health states, failing-drive alerts. Uses the `DiskHealth` platform trait;
//! honest "unknown" when SMART is unavailable (AGENTS.md risk table).

use hh_core::error::Result;
use hh_core::platform::DiskHealth;
use hh_core::time::now_ms;
use rusqlite::params;
use serde::Serialize;

use crate::{db_e, StorageService};

#[derive(Debug, Clone, Serialize)]
pub struct DiskHealthView {
    pub disk_id: String,
    pub model: Option<String>,
    pub health: String,
    pub temperature_c: Option<i32>,
    pub free_bytes: Option<u64>,
    pub last_checked_at: i64,
}

impl StorageService {
    /// Poll all disks and store snapshots (called every 6 h and on boot).
    pub fn collect_health(&self, disk_health: &dyn DiskHealth) -> Result<()> {
        for disk in disk_health.list().unwrap_or_default() {
            let report = disk_health.smart(&disk.id)?;
            {
                let c = self.db.lock()?;
                c.execute(
                    "INSERT INTO disks (id, model, serial, media_type, size_bytes, first_seen_at)
                     VALUES (?1,?2,?3,?4,?5,?6)
                     ON CONFLICT(id) DO UPDATE SET model=excluded.model, media_type=excluded.media_type",
                    params![
                        disk.id, disk.model, disk.serial, disk.media_type,
                        disk.size_bytes as i64, now_ms()
                    ],
                )
                .map_err(db_e)?;
                c.execute(
                    "INSERT INTO disk_health_snapshots
                     (id, disk_id, taken_at, health, predict_failure, temperature_c, power_on_hours,
                      reallocated_sectors, pending_sectors, free_bytes, raw_json)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    params![
                        ulid::Ulid::new().to_string(), disk.id, now_ms(), report.health,
                        report.predict_failure, report.temperature_c,
                        report.power_on_hours.map(|v| v as i64),
                        report.reallocated_sectors.map(|v| v as i64),
                        report.pending_sectors.map(|v| v as i64),
                        Option::<i64>::None, report.raw_json
                    ],
                )
                .map_err(db_e)?;
            }
            // FR-6.3: warn on failing drive; prompt immediate second copy.
            match report.health.as_str() {
                "failing" => {
                    self.db.create_alert(
                        "critical",
                        "DISK_FAILING",
                        "Your Home Hub's drive may be failing. Back up important photos to a second drive now.",
                    )?;
                }
                "caution" => {
                    self.db.create_alert(
                        "warning",
                        "DISK_CAUTION",
                        "Your Home Hub's drive shows early warning signs. Consider adding a second copy.",
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn health_summary(&self) -> Result<Vec<DiskHealthView>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare(
                "SELECT s.disk_id, d.model, s.health, s.temperature_c, s.free_bytes, s.taken_at
                 FROM disk_health_snapshots s
                 LEFT JOIN disks d ON d.id = s.disk_id
                 WHERE s.taken_at = (SELECT MAX(taken_at) FROM disk_health_snapshots WHERE disk_id = s.disk_id)",
            )
            .map_err(db_e)?;
        let rows = st
            .query_map([], |r| {
                Ok(DiskHealthView {
                    disk_id: r.get(0)?,
                    model: r.get(1)?,
                    health: r.get(2)?,
                    temperature_c: r.get(3)?,
                    free_bytes: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                    last_checked_at: r.get(5)?,
                })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        Ok(rows)
    }
}
