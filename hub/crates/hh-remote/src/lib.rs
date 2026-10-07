//! hh-remote: trackpad/keyboard/media/power (TRD §9, API_SPEC §8).
//!
//! Windows services run in Session 0 and cannot inject input — input and
//! power dispatch go through `InputControl`/`PowerControl` implemented by
//! the per-user `hh-session.exe` helper over a named pipe with restrictive
//! ACLs (see `helper_protocol` below).

pub mod helper_client;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use hh_core::error::{Error, Result};
use hh_core::platform::{InputControl, MediaKey, MouseButton, PowerControl};
use hh_core::time::now_ms;
use hh_db::Db;
use serde::Deserialize;

/// Rate limit: ≤500 msgs/s (API_SPEC §8).
const MAX_MSGS_PER_SEC: u64 = 500;
/// Idle timeout: session auto-ends after 60 s (API_SPEC §8).
pub const IDLE_TIMEOUT_MS: i64 = 60_000;

#[derive(Clone)]
pub struct RemoteService {
    pub db: Db,
    pub input: Arc<dyn InputControl>,
    pub power: Arc<dyn PowerControl>,
    pub enabled: bool,
    window_count: Arc<AtomicU64>,
    window_start_ms: Arc<AtomicU64>,
}

/// Incoming WebSocket input message (API_SPEC §8).
#[derive(Debug, Deserialize)]
#[serde(tag = "t")]
pub enum InputMsg {
    #[serde(rename = "mv")]
    Move { dx: i32, dy: i32 },
    #[serde(rename = "click")]
    Click { b: String, n: Option<u8> },
    #[serde(rename = "scroll")]
    Scroll { dy: i32 },
    #[serde(rename = "key")]
    Key { k: String, down: bool },
    #[serde(rename = "text")]
    Text { s: String },
}

impl RemoteService {
    pub fn new(db: Db, input: Arc<dyn InputControl>, power: Arc<dyn PowerControl>, enabled: bool) -> Self {
        Self {
            db,
            input,
            power,
            enabled,
            window_count: Arc::new(AtomicU64::new(0)),
            window_start_ms: Arc::new(AtomicU64::new(now_ms() as u64)),
        }
    }

    fn check_enabled(&self) -> Result<()> {
        if !self.enabled {
            return Err(Error::ForbiddenScope("remote".into()));
        }
        Ok(())
    }

    /// Sliding-window rate limiter; returns RateLimited when exceeded.
    fn rate_check(&self) -> Result<()> {
        let now = now_ms() as u64;
        let start = self.window_start_ms.load(Ordering::Relaxed);
        if now.saturating_sub(start) >= 1000 {
            self.window_start_ms.store(now, Ordering::Relaxed);
            self.window_count.store(0, Ordering::Relaxed);
        }
        let n = self.window_count.fetch_add(1, Ordering::Relaxed);
        if n >= MAX_MSGS_PER_SEC {
            return Err(Error::RateLimited);
        }
        Ok(())
    }

    pub fn handle_input(&self, device_id: &str, raw: &str) -> Result<()> {
        self.check_enabled()?;
        self.rate_check()?;
        let msg: InputMsg = serde_json::from_str(raw)
            .map_err(|e| Error::BadRequest(format!("bad input message: {e}")))?;
        match msg {
            InputMsg::Move { dx, dy } => self.input.mouse_move(dx.clamp(-500, 500), dy.clamp(-500, 500))?,
            InputMsg::Click { b, n } => {
                let btn = match b.as_str() {
                    "left" => MouseButton::Left,
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => return Err(Error::BadRequest("unknown button".into())),
                };
                self.input.click(btn, n.unwrap_or(1).min(2))?;
            }
            InputMsg::Scroll { dy } => self.input.scroll(dy.clamp(-40, 40))?,
            InputMsg::Key { k, down } => self.input.key(&k, down)?,
            InputMsg::Text { s } => {
                if s.chars().count() > 1000 {
                    return Err(Error::TooLarge("text input too long".into()));
                }
                self.input.text(&s)?;
            }
        }
        let _ = self.db.touch_device_seen(device_id, "");
        Ok(())
    }

    /// Power actions require explicit confirm and are audited (API_SPEC §8).
    pub fn power_action(&self, device_id: &str, action: &str, confirm: bool) -> Result<()> {
        self.check_enabled()?;
        if !confirm {
            return Err(Error::BadRequest("power actions require confirm:true".into()));
        }
        self.db.audit(Some(device_id), "power_action", Some(action), None)?;
        match action {
            "sleep" => self.power.sleep(),
            "restart" => self.power.restart(),
            "shutdown" => self.power.shutdown(),
            _ => Err(Error::BadRequest(format!("unknown power action {action}"))),
        }
    }

    pub fn media_key(&self, device_id: &str, key: &str) -> Result<()> {
        self.check_enabled()?;
        self.rate_check()?;
        let k = match key {
            "play_pause" => MediaKey::PlayPause,
            "next" => MediaKey::Next,
            "prev" => MediaKey::Prev,
            "vol_up" => MediaKey::VolUp,
            "vol_down" => MediaKey::VolDown,
            "mute" => MediaKey::Mute,
            _ => return Err(Error::BadRequest(format!("unknown media key {key}"))),
        };
        let _ = self.db.touch_device_seen(device_id, "");
        self.input.media_key(k)
    }

    pub fn record_session(&self, device_id: &str, kind: &str) -> Result<String> {
        let id = ulid::Ulid::new().to_string();
        let c = self.db.lock()?;
        c.execute(
            "INSERT INTO remote_sessions (id, device_id, kind, started_at) VALUES (?1,?2,?3,?4)",
            rusqlite::params![id, device_id, kind, now_ms()],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
        Ok(id)
    }

    pub fn end_session(&self, id: &str) -> Result<()> {
        let c = self.db.lock()?;
        c.execute(
            "UPDATE remote_sessions SET ended_at=?2 WHERE id=?1",
            rusqlite::params![id, now_ms()],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
        Ok(())
    }
}

/// Named-pipe IPC between `hh-service` (Session 0) and `hh-session.exe`
/// (user session). One JSON message per line, newline-delimited.
///
/// Pipe: `\\.\pipe\homehub-session` — ACL: service SID + interactive user
/// only. The helper authenticates the pipe client by PID→process path check.
pub mod helper_protocol {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "op")]
    pub enum HelperRequest {
        #[serde(rename = "mouse_move")]
        MouseMove { dx: i32, dy: i32 },
        #[serde(rename = "click")]
        Click { button: String, count: u8 },
        #[serde(rename = "scroll")]
        Scroll { dy: i32 },
        #[serde(rename = "key")]
        Key { key: String, down: bool },
        #[serde(rename = "text")]
        Text { s: String },
        #[serde(rename = "media")]
        Media { key: String },
        #[serde(rename = "power")]
        Power { action: String },
        #[serde(rename = "ping")]
        Ping,
        /// Deep hardware facts only the user session can see (WMI):
        /// battery health, camera presence, Wi-Fi standard (M3 audit).
        #[serde(rename = "system_info")]
        SystemInfo,
        /// Mobile-hotspot toggle (M3, FR-8.3) via WinRT tethering.
        #[serde(rename = "hotspot")]
        Hotspot { enable: bool, ssid: Option<String>, passphrase: Option<String> },
        /// BitLocker status for the library drive (FR-4.5 detect/recommend).
        #[serde(rename = "bitlocker")]
        BitlockerStatus,
        /// Screen sharing (M4): hand the WebRTC offer to the helper's media
        /// host; the answer + gathered host candidates come back in `data`.
        /// `kind` is "view" (laptop→phone send) or "cast" (phone→laptop recv).
        #[serde(rename = "screen_offer")]
        ScreenOffer { sdp: String, preset: String, kind: String },
        #[serde(rename = "add_ice")]
        AddIce { candidate: String },
        #[serde(rename = "screen_stop")]
        ScreenStop,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HelperResponse {
        pub ok: bool,
        #[serde(default)]
        pub error: Option<String>,
        /// Op-specific payload (system info, screen answer…).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<serde_json::Value>,
    }
}
