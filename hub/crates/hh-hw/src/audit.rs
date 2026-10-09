//! Hardware audit collection and capability ratings (TRD §11).

use hh_core::error::Result;
use hh_core::platform::{DiskInfo, HardwareReport};
use hh_core::time::now_ms;
use serde::Serialize;
use rusqlite::OptionalExtension;
use sysinfo::{Disks, Networks, System};

use crate::{db_e, HwService};

#[derive(Debug, Clone, Serialize)]
pub struct AuditResult {
    pub id: String,
    pub taken_at: i64,
    pub report: HardwareReport,
    pub ratings: Ratings,
    pub supported_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ratings {
    pub storage: &'static str,
    pub photo_backup: &'static str,
    pub file_sharing: &'static str,
    pub streaming: &'static str,
    pub local_ai: &'static str,
}

/// Collect hardware info cross-platform via sysinfo. Windows-specific deep
/// fields (Wi-Fi standard, battery health, HW encoders) come from WMI in the
/// hh-session helper; absent here they stay None and rate conservatively.
pub fn collect() -> Result<HardwareReport> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let cpu = sys.cpus().first();
    let disks = Disks::new_with_refreshed_list();
    let nets = Networks::new_with_refreshed_list();

    // Interface names do not prove link speed. Unknown until OS reports it.
    let ethernet_mbps = None;
    let wifi_standard = nets
        .iter()
        .find(|(name, _)| {
            let n = name.to_lowercase();
            n.contains("wi-fi") || n.contains("wifi") || n.contains("wlan")
        })
        .map(|_| "unknown".to_string());

    // x86-only CPUID macro; other architectures (incl. ARM Windows laptops)
    // report false and rely on the WMI audit in the hh-session helper.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let has_avx = std::arch::is_x86_feature_detected!("avx");
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let has_avx = false;

    Ok(HardwareReport {
        cpu_model: cpu.map(|c| c.brand().to_string()).unwrap_or_else(|| "unknown".into()),
        cpu_cores: sys.physical_core_count().unwrap_or(1) as u32,
        has_avx,
        ram_bytes: sys.total_memory(),
        disks: disks
            .iter()
            .map(|d| DiskInfo {
                id: d.name().to_string_lossy().to_string(),
                model: None,
                serial: None,
                media_type: "unknown".into(),
                size_bytes: d.total_space(),
            })
            .collect(),
        wifi_standard,
        ethernet_mbps,
        battery_health_pct: None, // WMI (Windows) / sysfs (Linux) in platform impl
        has_camera: false,        // probed by helper on Windows
        hw_encoders: vec![],
    })
}

/// Rating rules from TRD §11.
pub fn rate(report: &HardwareReport) -> Ratings {
    let storage = if report.disks.iter().any(|d| d.size_bytes >= 250 * 1024 * 1024 * 1024) {
        "excellent"
    } else if report.disks.iter().any(|d| d.size_bytes >= 120 * 1024 * 1024 * 1024) {
        "good"
    } else {
        "limited"
    };

    let photo_backup = match (report.ethernet_mbps, report.wifi_standard.as_deref()) {
        (Some(_), _) => "excellent",
        (_, Some("802.11ac")) | (_, Some("802.11ax")) => "excellent",
        (_, Some("802.11n")) => "good",
        _ => "limited",
    };

    let streaming = if !report.hw_encoders.is_empty()
        && matches!(photo_backup, "excellent")
    {
        "excellent"
    } else if report.cpu_cores >= 4 {
        "good"
    } else if report.cpu_cores >= 2 {
        "limited"
    } else {
        "not_recommended"
    };

    let local_ai = if report.ram_bytes >= 16 * 1024 * 1024 * 1024 {
        "excellent"
    } else {
        "not_recommended"
    };

    Ratings {
        storage,
        photo_backup,
        file_sharing: if photo_backup == "limited" { "limited" } else { "good" },
        streaming,
        local_ai,
    }
}

impl HwService {
    /// Run an audit, persist it, and derive feature flags (TRD §11).
    pub fn run_audit(&self) -> Result<AuditResult> {
        self.run_audit_with_helper(None)
    }

    pub fn run_audit_with_helper(&self,extra:Option<&serde_json::Value>) -> Result<AuditResult> {
        let mut report = collect()?;
        if let Some(extra)=extra {
            report.battery_health_pct=extra["battery_health_pct"].as_u64().and_then(|v|u32::try_from(v).ok());
            report.has_camera=extra["has_camera"].as_bool().unwrap_or(false);
            if let Some(standard)=extra["wifi_standard"].as_str(){report.wifi_standard=Some(standard.to_owned());}
        }
        let ratings = rate(&report);
        let supported_status = self.compat_status(&report)?;
        let id = ulid::Ulid::new().to_string();
        let c = self.db.lock()?;
        c.execute(
            "INSERT INTO hardware_audit
             (id, taken_at, report_json, rating_storage, rating_photo_backup, rating_file_sharing,
              rating_streaming, rating_local_ai, supported_status)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![
                id,
                now_ms(),
                serde_json::to_string(&report).map_err(|e| hh_core::Error::Internal(e.to_string()))?,
                ratings.storage, ratings.photo_backup, ratings.file_sharing,
                ratings.streaming, ratings.local_ai, supported_status
            ],
        )
        .map_err(db_e)?;
        Ok(AuditResult { id, taken_at: now_ms(), report, ratings, supported_status })
    }

    pub fn latest_audit(&self) -> Result<Option<serde_json::Value>> {
        let c = self.db.lock()?;
        let row = c
            .query_row(
                "SELECT id,taken_at,report_json,rating_storage,rating_photo_backup,rating_file_sharing,rating_streaming,rating_local_ai,supported_status FROM hardware_audit ORDER BY taken_at DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?)),
            )
            .optional().map_err(db_e)?;
        match row {
            Some((id,taken_at,report,storage,photo_backup,file_sharing,streaming,local_ai,supported_status)) => Ok(Some(serde_json::json!({
                "id":id,"taken_at":taken_at,"report":serde_json::from_str::<serde_json::Value>(&report).map_err(|e|hh_core::Error::Db(e.to_string()))?,
                "ratings":{"storage":storage,"photo_backup":photo_backup,"file_sharing":file_sharing,"streaming":streaming,"local_ai":local_ai},"supported_status":supported_status
            }))),
            None => Ok(None),
        }
    }
}
