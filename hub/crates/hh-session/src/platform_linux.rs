//! Linux session helper adapters. The input adapter retains its secure-surface
//! guard until a trusted owner-session controller explicitly unlocks it.
#![cfg(target_os = "linux")]

use hh_core::platform::{HwAudit, InputControl, MediaKey, MouseButton, PowerControl};
use hh_hw::linux::LinuxHwAudit;
use hh_remote::linux::{LinuxInputControl, LinuxPowerControl};
use once_cell::sync::Lazy;

static INPUT: Lazy<LinuxInputControl> = Lazy::new(LinuxInputControl::new);

pub fn release_input() -> Result<(), String> {
    INPUT.disconnect().map_err(|error| error.to_string())
}

pub fn mouse_move(dx: i32, dy: i32) -> Result<(), String> {
    INPUT.mouse_move(dx, dy).map_err(|error| error.to_string())
}

pub fn click(button: &str, count: u8) -> Result<(), String> {
    let button = match button {
        "left" => MouseButton::Left,
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => return Err(format!("unknown mouse button: {button}")),
    };
    INPUT.click(button, count).map_err(|error| error.to_string())
}

pub fn scroll(dy: i32) -> Result<(), String> {
    INPUT.scroll(dy).map_err(|error| error.to_string())
}

pub fn key(key: &str, down: bool) -> Result<(), String> {
    INPUT.key(key, down).map_err(|error| error.to_string())
}

pub fn text(s: &str) -> Result<(), String> {
    INPUT.text(s).map_err(|error| error.to_string())
}

pub fn media_key(key: &str) -> Result<(), String> {
    let key = match key {
        "play_pause" => MediaKey::PlayPause,
        "next" => MediaKey::Next,
        "prev" => MediaKey::Prev,
        "vol_up" => MediaKey::VolUp,
        "vol_down" => MediaKey::VolDown,
        "mute" => MediaKey::Mute,
        _ => return Err(format!("unknown media key: {key}")),
    };
    INPUT.media_key(key).map_err(|error| error.to_string())
}

pub fn power(action: &str) -> Result<(), String> {
    let control = LinuxPowerControl;
    let result = match action {
        "sleep" => control.sleep(),
        "restart" => control.restart(),
        "shutdown" => control.shutdown(),
        _ => return Err(format!("unknown power action: {action}")),
    };
    result.map_err(|error| error.to_string())
}

pub fn system_info() -> Result<serde_json::Value, String> {
    let report = LinuxHwAudit.collect().map_err(|error| error.to_string())?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

pub fn hotspot(_enable: bool, _ssid: Option<&str>, _pass: Option<&str>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"supported": false, "reason": "Linux hotspot control is unavailable"}))
}

pub fn bitlocker_status() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"supported": false, "reason": "Linux host"}))
}
