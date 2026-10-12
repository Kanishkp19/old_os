//! Live trial: a disposable RAM volume and a loopback-only sample endpoint.
//! The installer must re-enumerate disks and perform its own confirmation.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use thiserror::Error;

const OVERLAY: &str = "/run/homehub-trial";
const CONFIG: &str = "/etc/homehub/trial.json";
const MIB: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum TrialError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid trial configuration: {0}")]
    Config(#[from] serde_json::Error),
    #[error("trial safety check failed: {0}")]
    Unsafe(&'static str),
    #[error("invalid handoff preference")]
    Preference,
}
type Result<T> = std::result::Result<T, TrialError>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialConfig {
    pub trial_mode: bool,
    pub overlay_max_bytes: u64,
    pub allow_internal_disk_writes: bool,
    pub lan_advertising: bool,
    pub sample_data_path: PathBuf,
}
impl TrialConfig {
    pub fn quota(total_ram: u64) -> u64 {
        (512 * MIB).min(total_ram / 4)
    }
    pub fn load(path: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    pub fn validate(&self, total_ram: u64) -> Result<u64> {
        if !self.trial_mode || self.allow_internal_disk_writes || self.lan_advertising {
            return Err(TrialError::Unsafe(
                "trial policy must forbid disk writes and LAN advertising",
            ));
        }
        // 2 GiB is the minimum that leaves 512 MiB for the overlay and usable headroom.
        if total_ram < 2 * 1024 * MIB {
            return Err(TrialError::Unsafe("less than 2 GiB RAM"));
        }
        if self.overlay_max_bytes == 0
            || self.sample_data_path != Path::new("/usr/share/homehub/trial/samples")
        {
            return Err(TrialError::Unsafe("unexpected quota or sample path"));
        }
        Ok(self.overlay_max_bytes.min(Self::quota(total_ram)))
    }
}

pub struct DiskIsolation;
impl DiskIsolation {
    /// Read-only sysfs geometry. This API deliberately has no mount, unlock or write path.
    pub fn geometry() -> Result<Vec<(String, u64)>> {
        let mut disks = Vec::new();
        for entry in fs::read_dir("/sys/block")? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
                continue;
            }
            let sectors: u64 = fs::read_to_string(entry.path().join("size"))?
                .trim()
                .parse()
                .map_err(|_| TrialError::Unsafe("invalid disk geometry"))?;
            disks.push((name, sectors.saturating_mul(512)));
        }
        Ok(disks)
    }
    pub fn check_no_persistent_swap() -> Result<()> {
        for line in fs::read_to_string("/proc/swaps")?.lines().skip(1) {
            let source = line.split_whitespace().next().unwrap_or("");
            if !source.starts_with("/dev/zram") {
                return Err(TrialError::Unsafe("persistent or unknown swap is active"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TrialState {
    Unprepared,
    Active,
    LeavingForInstall,
    Stopped,
}
impl TrialState {
    pub fn leave_for_install(&mut self) -> Result<()> {
        if *self != Self::Active {
            return Err(TrialError::Unsafe("trial is not active"));
        }
        *self = Self::LeavingForInstall;
        Ok(())
    }
    pub fn cancel_install(&mut self) -> Result<()> {
        if *self != Self::LeavingForInstall {
            return Err(TrialError::Unsafe("installer was not entered"));
        }
        *self = Self::Active;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TrialHandoff {
    pub version: u8,
    pub language: String,
    pub input_mode: String,
    pub accessibility: String,
}
impl TrialHandoff {
    pub fn new(language: &str, input_mode: &str, accessibility: &str) -> Result<Self> {
        let handoff = Self {
            version: 1,
            language: language.into(),
            input_mode: input_mode.into(),
            accessibility: accessibility.into(),
        };
        handoff.validate()?;
        Ok(handoff)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || !["en", "hi"].contains(&self.language.as_str())
            || !["keyboard-mouse", "touch"].contains(&self.input_mode.as_str())
            || !["default", "large-text", "high-contrast", "screen-reader"]
                .contains(&self.accessibility.as_str())
        {
            return Err(TrialError::Preference);
        }
        Ok(())
    }
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(bytes)?;
        value.validate()?;
        Ok(value)
    }
}

fn total_ram() -> Result<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo")?;
    let kb = meminfo
        .lines()
        .find_map(|line| {
            line.strip_prefix("MemTotal:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
        .ok_or(TrialError::Unsafe("cannot measure RAM"))?;
    Ok(kb.saturating_mul(1024))
}
fn mounted_tmpfs(quota: u64) -> Result<bool> {
    Ok(fs::read_to_string("/proc/self/mountinfo")?
        .lines()
        .any(|line| {
            let mut fields = line.split(" - ");
            let before = fields.next().unwrap_or("");
            let after = fields.next().unwrap_or("");
            before.split_whitespace().nth(4) == Some(OVERLAY)
                && after.split_whitespace().next() == Some("tmpfs")
                && after
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or("")
                    .split(',')
                    .any(|option| {
                        option.strip_prefix("size=").and_then(parse_mount_size) == Some(quota)
                    })
        }))
}
fn parse_mount_size(size: &str) -> Option<u64> {
    let (number, scale) = match size.chars().last()? {
        'k' | 'K' => (&size[..size.len() - 1], 1024),
        'm' | 'M' => (&size[..size.len() - 1], MIB),
        'g' | 'G' => (&size[..size.len() - 1], 1024 * MIB),
        _ => (size, 1),
    };
    number.parse::<u64>().ok()?.checked_mul(scale)
}
fn prepare(quota: u64) -> Result<()> {
    DiskIsolation::check_no_persistent_swap()?;
    let _ = DiskIsolation::geometry()?;
    if mounted_tmpfs(quota)? {
        return Err(TrialError::Unsafe("trial overlay already mounted"));
    }
    fs::create_dir_all(OVERLAY)?;
    let status = Command::new("/bin/mount")
        .args([
            "-t",
            "tmpfs",
            "-o",
            &format!("size={quota},mode=0700,nosuid,nodev,noexec"),
            "tmpfs",
            OVERLAY,
        ])
        .status()?;
    if !status.success() || !mounted_tmpfs(quota)? {
        return Err(TrialError::Unsafe("RAM overlay mount failed"));
    }
    Ok(())
}
fn serve(quota: u64) -> Result<()> {
    if !mounted_tmpfs(quota)? {
        return Err(TrialError::Unsafe("RAM overlay is absent or exceeds quota"));
    }
    DiskIsolation::check_no_persistent_swap()?;
    // The only reachable endpoint is inside systemd's private network namespace.
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 47801))?;
    let manifest = include_bytes!("../../trial/sample-manifest.json").as_slice();
    if manifest.len() > MIB as usize {
        return Err(TrialError::Unsafe("sample manifest too large"));
    }
    let _: serde_json::Value = serde_json::from_slice(manifest)?;
    tracing::info!(quota, "trial sample endpoint ready");
    for stream in listener.incoming() {
        let mut stream = stream?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut request = [0u8; 1024];
        let len = stream.read(&mut request)?;
        let (status, body) = if request[..len].starts_with(b"GET /samples HTTP/1.1\r\n") {
            ("200 OK", manifest)
        } else {
            ("404 Not Found", b"{}".as_slice())
        };
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())?;
        stream.write_all(body)?;
    }
    Ok(())
}
fn main() -> Result<()> {
    let config = TrialConfig::load(Path::new(CONFIG))?;
    let quota = config.validate(total_ram()?)?;
    match std::env::args().nth(1).as_deref() {
        Some("prepare") => prepare(quota),
        Some("serve") => serve(quota),
        _ => Err(TrialError::Unsafe("expected prepare or serve")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_and_handoff_are_bounded() {
        assert_eq!(TrialConfig::quota(1024 * MIB), 256 * MIB);
        assert_eq!(TrialConfig::quota(8 * 1024 * MIB), 512 * MIB);
        assert!(TrialHandoff::from_json(br#"{"version":1,"language":"en","input_mode":"touch","accessibility":"default","notes":"secret"}"#).is_err());
        assert!(TrialHandoff::new("hi", "keyboard-mouse", "screen-reader").is_ok());
    }
}
