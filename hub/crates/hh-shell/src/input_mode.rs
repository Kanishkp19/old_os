//! Owner input preference and physical-device boot detection.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, io, path::{Path, PathBuf}};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    #[default]
    Auto,
    Mouse,
    Touch,
}

#[derive(Clone, Copy, Default)]
struct Devices {
    touchscreen: bool,
    pointer: bool,
    tablet_posture: bool,
}

impl Devices {
    fn selected(self) -> InputMode {
        if self.touchscreen && (self.tablet_posture || !self.pointer) {
            InputMode::Touch
        } else {
            InputMode::Mouse
        }
    }
}

/// Detect the layout after honoring an explicit owner preference. Automatic discovery waits at most two seconds.
pub fn detect() -> InputMode {
    let path = owner_preferences_path();
    match read_preference(&path).unwrap_or(InputMode::Auto) {
        InputMode::Mouse => InputMode::Mouse,
        InputMode::Touch => InputMode::Touch,
        InputMode::Auto => detect_auto(),
    }
}

pub fn owner_preferences_path() -> PathBuf {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state).join("homehub/shell.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/homehub/shell.json");
    }
    PathBuf::from("shell.json")
}

pub fn read_preference(path: &Path) -> io::Result<InputMode> {
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    match value.pointer("/input/mode").and_then(Value::as_str) {
        Some("mouse") => Ok(InputMode::Mouse),
        Some("touch") => Ok(InputMode::Touch),
        _ => Ok(InputMode::Auto),
    }
}

pub fn write_preference(path: &Path, mode: InputMode) -> io::Result<()> {
    let mut value: Value = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => json!({}),
        Err(error) => return Err(error),
    };
    if !value.is_object() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "owner shell JSON must be an object"));
    }
    value["input"] = match value.get("input") {
        Some(existing) if existing.is_object() => existing.clone(),
        _ => json!({}),
    };
    value["input"]["mode"] = serde_json::to_value(mode)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&value)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> io::Result<()> {
        use std::io::Write;
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn detect_auto() -> InputMode {
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut seen = Devices::default();
        loop {
            let current = probe_linux();
            seen.touchscreen |= current.touchscreen;
            seen.pointer |= current.pointer;
            seen.tablet_posture = current.tablet_posture;
            if Instant::now() >= deadline {
                return seen.selected();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Devices::default().selected()
    }
}

#[cfg(target_os = "linux")]
fn bit_set(bitmap: &str, bit: usize) -> bool {
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    let word_index = bit / 64;
    let Some(word) = words.iter().rev().nth(word_index) else { return false; };
    u64::from_str_radix(word, 16).is_ok_and(|value| value & (1u64 << (bit % 64)) != 0)
}

#[cfg(target_os = "linux")]
fn capability(path: &Path, kind: &str, bit: usize) -> bool {
    fs::read_to_string(path.join("device/capabilities").join(kind))
        .is_ok_and(|value| bit_set(&value, bit))
}

#[cfg(target_os = "linux")]
fn probe_linux() -> Devices {
    let mut devices = Devices::default();
    let Ok(entries) = fs::read_dir("/sys/class/input") else { return devices; };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else { continue; };
        if !name.starts_with("event") { continue; }
        let Ok(physical) = fs::canonicalize(&path) else { continue; };
        if physical.to_string_lossy().contains("/virtual/") { continue; }
        let label = fs::read_to_string(path.join("device/name")).unwrap_or_default().to_lowercase();
        if label.contains("uinput") || label.contains("virtual") { continue; }
        let touch = capability(&path, "key", 330)
            && capability(&path, "abs", 53)
            && capability(&path, "abs", 54);
        let mouse = capability(&path, "key", 272)
            && capability(&path, "rel", 0)
            && capability(&path, "rel", 1);
        let trackpad = capability(&path, "key", 325)
            && capability(&path, "abs", 0)
            && capability(&path, "abs", 1);
        devices.touchscreen |= touch;
        devices.pointer |= mouse || trackpad;
        if capability(&path, "sw", 1) {
            devices.tablet_posture |= tablet_switch_active(name);
        }
    }
    devices
}

#[cfg(target_os = "linux")]
fn tablet_switch_active(event: &str) -> bool {
    use std::{fs::File, os::fd::AsRawFd};
    let Ok(file) = File::open(Path::new("/dev/input").join(event)) else { return false; };
    let mut switches: u64 = 0;
    // EVIOCGSW(8): Linux input ioctl reading the current switch bitmap.
    let result = unsafe { libc::ioctl(file.as_raw_fd(), 0x8008_451b_u64 as libc::c_ulong, &mut switches) };
    result >= 0 && switches & (1 << 1) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_round_trip_preserves_other_owner_settings() {
        let path = std::env::temp_dir().join(format!("hh-shell-pref-{}.json", std::process::id()));
        fs::write(&path, br#"{"language":"hi","input":{"mode":"auto","hint":true}}"#).unwrap();
        write_preference(&path, InputMode::Touch).unwrap();
        assert_eq!(read_preference(&path).unwrap(), InputMode::Touch);
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["language"], "hi");
        assert_eq!(value["input"]["hint"], true);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn automatic_selection() {
        assert_eq!(Devices { touchscreen: true, ..Devices::default() }.selected(), InputMode::Touch);
        assert_eq!(Devices { touchscreen: true, pointer: true, ..Devices::default() }.selected(), InputMode::Mouse);
        assert_eq!(Devices { touchscreen: true, pointer: true, tablet_posture: true }.selected(), InputMode::Touch);
        assert_eq!(Devices::default().selected(), InputMode::Mouse);
    }
}
