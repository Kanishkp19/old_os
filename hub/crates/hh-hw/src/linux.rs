//! Linux hardware inventory from procfs/sysfs with bounded SMART probing.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use hh_core::error::{Error, Result};
use hh_core::platform::{DiskHealth, DiskInfo, HardwareReport, HwAudit, SmartReport};

pub struct LinuxDiskHealth;
pub struct LinuxHwAudit;

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn partitions() -> Result<HashSet<String>> {
    let text = fs::read_to_string("/proc/partitions")?;
    Ok(text.lines().skip(2).filter_map(|line| line.split_whitespace().nth(3))
        .map(str::to_owned).collect())
}

impl DiskHealth for LinuxDiskHealth {
    fn list(&self) -> Result<Vec<DiskInfo>> {
        let known = partitions()?;
        let mut disks = Vec::new();
        for entry in fs::read_dir("/sys/block")? {
            let entry = entry?;
            let id = entry.file_name().to_string_lossy().into_owned();
            if !known.contains(&id) || id.starts_with("loop") || id.starts_with("ram") {
                continue;
            }
            let path = entry.path();
            let sectors = read_trimmed(path.join("size"))
                .and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let rotational = read_trimmed(path.join("queue/rotational"));
            let media_type = if id.starts_with("nvme") { "nvme" }
                else if id.starts_with("sd") && read_trimmed(path.join("removable")).as_deref() == Some("1") { "usb" }
                else if rotational.as_deref() == Some("1") { "hdd" }
                else if rotational.as_deref() == Some("0") { "ssd" }
                else { "unknown" };
            disks.push(DiskInfo {
                id,
                model: read_trimmed(path.join("device/model")).filter(|s| !s.is_empty()),
                serial: read_trimmed(path.join("device/serial")).filter(|s| !s.is_empty()),
                media_type: media_type.into(),
                size_bytes: sectors.saturating_mul(512),
            });
        }
        disks.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(disks)
    }

    fn smart(&self, id: &str) -> Result<SmartReport> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !self.list()?.iter().any(|disk| disk.id == id)
        {
            return Err(Error::BadRequest("unknown block device".into()));
        }
        let path = format!("/dev/{id}");
        let mut child = match Command::new("smartctl")
            .args(["-j", "-H", "-A", "--", &path])
            .stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(unknown_smart()),
            Err(e) => return Err(e.into()),
        };
        let start = Instant::now();
        loop {
            if child.try_wait()?.is_some() { break; }
            if start.elapsed() >= Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Internal("smartctl timed out".into()));
            }
            thread::sleep(Duration::from_millis(25));
        }
        let mut bytes = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            stdout.take(128 * 1024 + 1).read_to_end(&mut bytes)?;
        }
        if bytes.len() > 128 * 1024 {
            return Err(Error::TooLarge("smartctl output exceeds 128 KiB".into()));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Internal(format!("smartctl JSON: {e}")))?;
        let passed = value.pointer("/smart_status/passed").and_then(|v| v.as_bool());
        let warning = value.pointer("/nvme_smart_health_information_log/critical_warning")
            .and_then(|v| v.as_u64());
        let predict_failure = passed.map(|ok| !ok).or_else(|| warning.map(|n| n != 0));
        let health = match predict_failure {
            Some(false) => "good",
            Some(true) => "failing",
            None => "unknown",
        };
        let attribute = |name: &str| -> Option<u64> {
            value.pointer("/ata_smart_attributes/table")?.as_array()?.iter()
                .find(|row| row.get("name").and_then(|v| v.as_str()) == Some(name))?
                .pointer("/raw/value")?.as_u64()
        };
        Ok(SmartReport {
            health: health.into(),
            predict_failure,
            temperature_c: value.pointer("/temperature/current").and_then(|v| v.as_i64())
                .and_then(|n| i32::try_from(n).ok()),
            power_on_hours: value.pointer("/power_on_time/hours").and_then(|v| v.as_u64()),
            reallocated_sectors: attribute("Reallocated_Sector_Ct"),
            pending_sectors: attribute("Current_Pending_Sector"),
            raw_json: Some(String::from_utf8_lossy(&bytes).into_owned()),
        })
    }
}

fn unknown_smart() -> SmartReport {
    SmartReport {
        health: "unknown".into(), predict_failure: None, temperature_c: None,
        power_on_hours: None, reallocated_sectors: None, pending_sectors: None,
        raw_json: None,
    }
}

impl HwAudit for LinuxHwAudit {
    fn collect(&self) -> Result<HardwareReport> {
        let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        let cpu_model = cpuinfo.lines().find_map(|line| line.strip_prefix("model name"))
            .and_then(|value| value.split_once(':').map(|(_, value)| value.trim().to_owned()))
            .unwrap_or_else(|| "unknown".into());
        let cpu_cores = cpuinfo.lines().filter(|line| line.starts_with("processor")).count();
        let ram_kib = meminfo.lines().find_map(|line| line.strip_prefix("MemTotal:"))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok()).unwrap_or(0);
        let mut wifi_standard = None;
        let mut ethernet_mbps = None;
        for entry in fs::read_dir("/sys/class/net")? {
            let entry = entry?;
            let path = entry.path();
            if path.join("wireless").exists() || path.join("phy80211").exists() {
                wifi_standard = Some("unknown".to_owned());
            } else if entry.file_name() != "lo" {
                let speed = read_trimmed(path.join("speed")).and_then(|s| s.parse::<u32>().ok());
                ethernet_mbps = ethernet_mbps.max(speed);
            }
        }
        let battery_health_pct = fs::read_dir("/sys/class/power_supply").ok().and_then(|entries| {
            entries.filter_map(std::result::Result::ok).find_map(|entry| {
                let path = entry.path();
                if !entry.file_name().to_string_lossy().starts_with("BAT") { return None; }
                let full = read_trimmed(path.join("energy_full")).or_else(|| read_trimmed(path.join("charge_full")))?.parse::<u64>().ok()?;
                let design = read_trimmed(path.join("energy_full_design")).or_else(|| read_trimmed(path.join("charge_full_design")))?.parse::<u64>().ok()?;
                (design > 0).then(|| ((full.saturating_mul(100) / design).min(100)) as u32)
            })
        });
        Ok(HardwareReport {
            cpu_model,
            cpu_cores: u32::try_from(cpu_cores.max(1)).unwrap_or(u32::MAX),
            has_avx: cpuinfo.lines().any(|line| line.starts_with("flags") && line.split_whitespace().any(|word| word == "avx")),
            ram_bytes: ram_kib.saturating_mul(1024),
            disks: LinuxDiskHealth.list()?,
            wifi_standard,
            ethernet_mbps,
            battery_health_pct,
            has_camera: Path::new("/dev/video0").exists(),
            hw_encoders: Vec::new(),
        })
    }
}
