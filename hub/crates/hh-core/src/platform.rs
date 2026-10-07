//! Platform abstraction traits (TRD §16). Phase 4 ports these to Linux;
//! call sites never change. Windows implementations live in hh-service
//! (`hh-session.exe` helper for anything Session 0 cannot do, TRD §9).

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub id: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub media_type: String, // hdd | ssd | nvme | usb | unknown
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartReport {
    pub health: String, // good | caution | failing | unknown
    pub predict_failure: Option<bool>,
    pub temperature_c: Option<i32>,
    pub power_on_hours: Option<u64>,
    pub reallocated_sectors: Option<u64>,
    pub pending_sectors: Option<u64>,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareReport {
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub has_avx: bool,
    pub ram_bytes: u64,
    pub disks: Vec<DiskInfo>,
    pub wifi_standard: Option<String>,
    pub ethernet_mbps: Option<u32>,
    pub battery_health_pct: Option<u32>,
    pub has_camera: bool,
    pub hw_encoders: Vec<String>, // e.g. ["h264_nvenc","h264_qsv"]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub trait PowerControl: Send + Sync {
    fn sleep(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    fn shutdown(&self) -> Result<()>;
}

pub trait InputControl: Send + Sync {
    fn mouse_move(&self, dx: i32, dy: i32) -> Result<()>;
    fn click(&self, btn: MouseButton, count: u8) -> Result<()>;
    fn scroll(&self, dy: i32) -> Result<()>;
    fn key(&self, key: &str, down: bool) -> Result<()>;
    fn text(&self, s: &str) -> Result<()>;
    fn media_key(&self, key: MediaKey) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKey {
    PlayPause,
    Next,
    Prev,
    VolUp,
    VolDown,
    Mute,
}

pub trait DiskHealth: Send + Sync {
    fn list(&self) -> Result<Vec<DiskInfo>>;
    fn smart(&self, id: &str) -> Result<SmartReport>;
}

pub trait HwAudit: Send + Sync {
    fn collect(&self) -> Result<HardwareReport>;
}

/// Service host: Windows SCM service or plain console process (M0).
pub trait ServiceHost: Send + Sync {
    fn run(self: Box<Self>) -> Result<()>;
}

/// No-op implementations used on unix dev builds and in tests.
pub mod noop {
    use super::*;
    use crate::error::Error;

    pub struct Unsupported;
    fn unsupported() -> Error {
        Error::Internal("operation not supported on this platform".into())
    }

    impl PowerControl for Unsupported {
        fn sleep(&self) -> Result<()> { Err(unsupported()) }
        fn restart(&self) -> Result<()> { Err(unsupported()) }
        fn shutdown(&self) -> Result<()> { Err(unsupported()) }
    }
    impl InputControl for Unsupported {
        fn mouse_move(&self, _dx: i32, _dy: i32) -> Result<()> { Err(unsupported()) }
        fn click(&self, _b: MouseButton, _n: u8) -> Result<()> { Err(unsupported()) }
        fn scroll(&self, _dy: i32) -> Result<()> { Err(unsupported()) }
        fn key(&self, _k: &str, _down: bool) -> Result<()> { Err(unsupported()) }
        fn text(&self, _s: &str) -> Result<()> { Err(unsupported()) }
        fn media_key(&self, _k: MediaKey) -> Result<()> { Err(unsupported()) }
    }
    impl DiskHealth for Unsupported {
        fn list(&self) -> Result<Vec<DiskInfo>> { Ok(vec![]) }
        fn smart(&self, _id: &str) -> Result<SmartReport> {
            Ok(SmartReport {
                health: "unknown".into(),
                predict_failure: None,
                temperature_c: None,
                power_on_hours: None,
                reallocated_sectors: None,
                pending_sectors: None,
                raw_json: None,
            })
        }
    }
}
