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

/// Windows Storage Management health; unsupported SMART fields remain None.
/// Uses Microsoft's local Get-PhysicalDisk and Get-StorageReliabilityCounter.
pub struct NativeDiskHealth;
impl DiskHealth for NativeDiskHealth {
    fn list(&self)->Result<Vec<hh_core::platform::DiskInfo>> {
        #[cfg(windows)] {
            if let Ok(rows)=physical_disks(){return Ok(rows.iter().map(|r|hh_core::platform::DiskInfo{
                id:r["UniqueId"].as_str().unwrap_or("unknown").to_string(),model:r["FriendlyName"].as_str().map(str::to_string),serial:r["SerialNumber"].as_str().map(str::to_string),media_type:match r["MediaType"].as_u64(){Some(3)=>"hdd",Some(4)=>"ssd",_=>"unknown"}.into(),size_bytes:r["Size"].as_u64().unwrap_or(0)}).collect());}
        }
        Ok(sysinfo::Disks::new_with_refreshed_list().iter().map(|d|hh_core::platform::DiskInfo{id:d.mount_point().to_string_lossy().to_string(),model:Some(d.name().to_string_lossy().to_string()),serial:None,media_type:"unknown".into(),size_bytes:d.total_space()}).collect())
    }
    fn smart(&self,id:&str)->Result<hh_core::platform::SmartReport> {
        let mut report=hh_core::platform::SmartReport{health:"unknown".into(),predict_failure:None,temperature_c:None,power_on_hours:None,reallocated_sectors:None,pending_sectors:None,raw_json:None};
        #[cfg(not(windows))] let _=id;
        #[cfg(windows)] if let Ok(rows)=physical_disks(){if let Some(row)=rows.iter().find(|r|r["UniqueId"].as_str()==Some(id)) {
            // Numeric enum values are emitted by ConvertTo-Json.
            report.health=match row["HealthStatus"].as_u64(){Some(0)=>"good",Some(1)=>"caution",Some(2)=>"failing",_=>"unknown"}.into();
            report.temperature_c=row["Temperature"].as_i64().filter(|n|*n>0).map(|n|n as i32);
            report.power_on_hours=row["PowerOnHours"].as_u64();report.raw_json=Some(row.to_string());
        }}
        Ok(report)
    }
}
#[cfg(windows)]
fn physical_disks()->Result<Vec<serde_json::Value>> {
    use std::process::{Command,Stdio};use std::io::Read;use std::os::windows::process::CommandExt;
    let script="$ErrorActionPreference='Stop'; @((Get-PhysicalDisk | ForEach-Object { $d=$_; $r=$null; try {$r=$d | Get-StorageReliabilityCounter} catch {}; [pscustomobject]@{UniqueId=$d.UniqueId;FriendlyName=$d.FriendlyName;SerialNumber=$d.SerialNumber;Size=$d.Size;MediaType=[int]$d.MediaType;HealthStatus=[int]$d.HealthStatus;Temperature=$r.Temperature;PowerOnHours=$r.PowerOnHours} })) | ConvertTo-Json -Compress";
    let mut child=Command::new("powershell.exe").args(["-NoLogo","-NoProfile","-NonInteractive","-Command",script]).creation_flags(0x08000000).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let start=std::time::Instant::now();loop{if let Some(status)=child.try_wait()?{if !status.success(){return Err(hh_core::Error::StorageUnavailable("disk health provider unavailable".into()));}break;}if start.elapsed()>std::time::Duration::from_secs(15){let _=child.kill();let _=child.wait();return Err(hh_core::Error::StorageUnavailable("disk health provider timeout".into()));}std::thread::sleep(std::time::Duration::from_millis(50));}
    let mut bytes=Vec::new();if let Some(stdout)=child.stdout.take(){stdout.take(1024*1024).read_to_end(&mut bytes)?;}
    let value:serde_json::Value=serde_json::from_slice(&bytes).map_err(|e|hh_core::Error::Internal(e.to_string()))?;
    Ok(match value {serde_json::Value::Array(v)=>v,serde_json::Value::Object(_)=>vec![value],_=>Vec::new()})
}
