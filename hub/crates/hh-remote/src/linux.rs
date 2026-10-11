//! Linux input and power adapters. Callers authorize remote sessions before reaching these traits.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::process::Command;
use std::sync::Mutex;

use hh_core::error::{Error, Result};
use hh_core::platform::{InputControl, MediaKey, MouseButton, PowerControl};

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;
const REL_WHEEL: u16 = 8;
const SYN_REPORT: u16 = 0;
const BTN_LEFT: u16 = 272;
const BTN_RIGHT: u16 = 273;
const BTN_MIDDLE: u16 = 274;
const BUS_USB: u16 = 3;
const UI_SET_EVBIT: usize = 0x4004_5564;
const UI_SET_KEYBIT: usize = 0x4004_5565;
const UI_SET_RELBIT: usize = 0x4004_5566;
const UI_DEV_CREATE: usize = 0x5501;
const UI_DEV_DESTROY: usize = 0x5502;

unsafe extern "C" {
    fn ioctl(fd: i32, request: usize, ...) -> i32;
}

struct InputDevice {
    file: Option<File>,
    held: HashSet<u16>,
    secure_surface_active: bool,
}

impl InputDevice {
    fn open() -> std::io::Result<Self> {
        let mut file = OpenOptions::new().write(true).open("/dev/uinput")?;
        let fd = file.as_raw_fd();
        set_bit(fd, UI_SET_EVBIT, EV_SYN)?;
        set_bit(fd, UI_SET_EVBIT, EV_KEY)?;
        set_bit(fd, UI_SET_EVBIT, EV_REL)?;
        for code in 1..=255 {
            set_bit(fd, UI_SET_KEYBIT, code)?;
        }
        for code in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE] {
            set_bit(fd, UI_SET_KEYBIT, code)?;
        }
        for code in [REL_X, REL_Y, REL_WHEEL] {
            set_bit(fd, UI_SET_RELBIT, code)?;
        }

        // Linux uinput_user_dev: name[80], input_id[8], ff_effects_max[4],
        // four arrays of 64 i32 values. Zeroed absolute axes are unused.
        let mut setup = vec![0u8; 80 + 8 + 4 + 4 * 64 * 4];
        let name = b"HomeHub remote input";
        setup[..name.len()].copy_from_slice(name);
        setup[80..82].copy_from_slice(&BUS_USB.to_ne_bytes());
        setup[82..84].copy_from_slice(&0x4848u16.to_ne_bytes());
        setup[84..86].copy_from_slice(&0x0001u16.to_ne_bytes());
        setup[86..88].copy_from_slice(&0x0001u16.to_ne_bytes());
        file.write_all(&setup)?;
        ioctl_no_arg(fd, UI_DEV_CREATE)?;
        Ok(Self { file: Some(file), held: HashSet::new(), secure_surface_active: true })
    }

    fn emit(&mut self, kind: u16, code: u16, value: i32) -> Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            Error::Internal("remote input unavailable: /dev/uinput access denied".into())
        })?;
        let mut event = vec![0u8; 2 * std::mem::size_of::<isize>()];
        event.extend_from_slice(&kind.to_ne_bytes());
        event.extend_from_slice(&code.to_ne_bytes());
        event.extend_from_slice(&value.to_ne_bytes());
        file.write_all(&event)?;
        Ok(())
    }

    fn sync(&mut self) -> Result<()> {
        self.emit(EV_SYN, SYN_REPORT, 0)
    }

    fn key_event(&mut self, code: u16, down: bool) -> Result<()> {
        self.emit(EV_KEY, code, i32::from(down))?;
        if down { self.held.insert(code); } else { self.held.remove(&code); }
        self.sync()
    }

    fn release_all(&mut self) -> Result<()> {
        let held: Vec<u16> = self.held.drain().collect();
        for code in held {
            self.emit(EV_KEY, code, 0)?;
        }
        self.sync()
    }
}

fn set_bit(fd: i32, request: usize, code: u16) -> std::io::Result<()> {
    // SAFETY: fd is a live /dev/uinput descriptor; these ioctl requests take an integer code.
    if unsafe { ioctl(fd, request, i32::from(code)) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn ioctl_no_arg(fd: i32, request: usize) -> std::io::Result<()> {
    // SAFETY: UI_DEV_CREATE/DESTROY require no extra argument.
    if unsafe { ioctl(fd, request) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

impl Drop for InputDevice {
    fn drop(&mut self) {
        if self.file.is_some() {
            if let Err(error) = self.release_all() {
                tracing::warn!(%error, "failed to release remote input keys");
            }
            if let Some(file) = &self.file {
                if let Err(error) = ioctl_no_arg(file.as_raw_fd(), UI_DEV_DESTROY) {
                    tracing::warn!(%error, "failed to destroy uinput device");
                }
            }
        }
    }
}

pub struct LinuxInputControl {
    device: Mutex<InputDevice>,
}

impl Default for LinuxInputControl {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxInputControl {
    pub fn new() -> Self {
        let device = match InputDevice::open() {
            Ok(device) => device,
            Err(error) => {
                tracing::warn!(%error, "remote input disabled; /dev/uinput is unavailable");
                InputDevice { file: None, held: HashSet::new(), secure_surface_active: true }
            }
        };
        Self { device: Mutex::new(device) }
    }

    pub fn disconnect(&self) -> Result<()> {
        self.lock()?.release_all()
    }

    /// The trusted owner-session controller must clear this before remote input is usable.
    /// Entering a lock or credential surface revokes all held keys under the same mutex.
    pub fn set_secure_surface_active(&self, active: bool) -> Result<()> {
        let mut device = self.lock()?;
        device.secure_surface_active = active;
        if active { device.release_all()?; }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, InputDevice>> {
        self.device.lock().map_err(|_| Error::Internal("remote input state poisoned".into()))
    }

    fn active_device(&self) -> Result<std::sync::MutexGuard<'_, InputDevice>> {
        let device = self.lock()?;
        if device.secure_surface_active {
            return Err(Error::ForbiddenScope("remote input blocked on secure surface".into()));
        }
        Ok(device)
    }
}

impl InputControl for LinuxInputControl {
    fn mouse_move(&self, dx: i32, dy: i32) -> Result<()> {
        let mut device = self.active_device()?;
        device.emit(EV_REL, REL_X, dx.clamp(-500, 500))?;
        device.emit(EV_REL, REL_Y, dy.clamp(-500, 500))?;
        device.sync()
    }

    fn click(&self, btn: MouseButton, count: u8) -> Result<()> {
        if count == 0 || count > 2 {
            return Err(Error::BadRequest("click count must be 1 or 2".into()));
        }
        let code = match btn {
            MouseButton::Left => BTN_LEFT,
            MouseButton::Right => BTN_RIGHT,
            MouseButton::Middle => BTN_MIDDLE,
        };
        let mut device = self.active_device()?;
        for _ in 0..count {
            device.key_event(code, true)?;
            device.key_event(code, false)?;
        }
        Ok(())
    }

    fn scroll(&self, dy: i32) -> Result<()> {
        let mut device = self.active_device()?;
        device.emit(EV_REL, REL_WHEEL, dy.clamp(-40, 40))?;
        device.sync()
    }

    fn key(&self, key: &str, down: bool) -> Result<()> {
        let code = key_code(key).ok_or_else(|| Error::BadRequest(format!("unsupported key: {key}")))?;
        self.active_device()?.key_event(code, down)
    }

    fn text(&self, s: &str) -> Result<()> {
        if s.chars().count() > 1000 {
            return Err(Error::TooLarge("text input too long".into()));
        }
        let sequence: Vec<(u16, bool)> = s.chars().map(text_key).collect::<Option<_>>()
            .ok_or_else(|| Error::BadRequest("non-ASCII text requires the owner-session IME".into()))?;
        let mut device = self.active_device()?;
        if [42, 29, 56, 125].iter().any(|code| device.held.contains(code)) {
            return Err(Error::BadRequest("release modifiers before text injection".into()));
        }
        for (code, shift) in sequence {
            if shift { device.key_event(42, true)?; }
            device.key_event(code, true)?;
            device.key_event(code, false)?;
            if shift { device.key_event(42, false)?; }
        }
        Ok(())
    }

    fn media_key(&self, key: MediaKey) -> Result<()> {
        let code = match key {
            MediaKey::PlayPause => 164,
            MediaKey::Next => 163,
            MediaKey::Prev => 165,
            MediaKey::VolUp => 115,
            MediaKey::VolDown => 114,
            MediaKey::Mute => 113,
        };
        let mut device = self.active_device()?;
        device.key_event(code, true)?;
        device.key_event(code, false)
    }
}

fn key_code(key: &str) -> Option<u16> {
    let lower = key.to_ascii_lowercase();
    let named = match lower.as_str() {
        "enter" | "return" => 28, "backspace" => 14, "tab" => 15,
        "escape" | "esc" => 1, "space" => 57, "up" => 103, "down" => 108,
        "left" => 105, "right" => 106, "home" => 102, "end" => 107,
        "pageup" => 104, "pagedown" => 109, "delete" | "del" => 111,
        "insert" => 110, "shift" => 42, "ctrl" | "control" => 29,
        "alt" => 56, "meta" | "win" => 125, "menu" => 127,
        _ => {
            if let Some(number) = lower.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
                return (1..=10).contains(&number).then_some(58 + number)
                    .or_else(|| (11..=12).contains(&number).then_some(76 + number));
            }
            return lower.chars().next().filter(|_| lower.chars().count() == 1)
                .and_then(|c| text_key(c).map(|(code, _)| code));
        }
    };
    Some(named)
}

fn text_key(c: char) -> Option<(u16, bool)> {
    let shifted = c.is_ascii_uppercase();
    let lower = c.to_ascii_lowercase();
    let code = match lower {
        'a' => 30, 'b' => 48, 'c' => 46, 'd' => 32, 'e' => 18, 'f' => 33,
        'g' => 34, 'h' => 35, 'i' => 23, 'j' => 36, 'k' => 37, 'l' => 38,
        'm' => 50, 'n' => 49, 'o' => 24, 'p' => 25, 'q' => 16, 'r' => 19,
        's' => 31, 't' => 20, 'u' => 22, 'v' => 47, 'w' => 17, 'x' => 45,
        'y' => 21, 'z' => 44, '1' | '!' => 2, '2' | '@' => 3,
        '3' | '#' => 4, '4' | '$' => 5, '5' | '%' => 6,
        '6' | '^' => 7, '7' | '&' => 8, '8' | '*' => 9,
        '9' | '(' => 10, '0' | ')' => 11, ' ' => 57,
        '-' | '_' => 12, '=' | '+' => 13, '[' | '{' => 26,
        ']' | '}' => 27, ';' | ':' => 39, '\'' | '"' => 40,
        '`' | '~' => 41, '\\' | '|' => 43, ',' | '<' => 51,
        '.' | '>' => 52, '/' | '?' => 53, '\n' => 28, '\t' => 15,
        _ => return None,
    };
    Some((code, shifted || "!@#$%^&*()_+{}:\"~|<>?".contains(c)))
}

pub struct LinuxPowerControl;

impl LinuxPowerControl {
    fn run(action: &str) -> Result<()> {
        let status = Command::new("systemctl").args(["--no-ask-password", action]).status()?;
        if status.success() { Ok(()) } else { Err(Error::Internal(format!("systemctl {action} failed: {status}"))) }
    }
}

impl PowerControl for LinuxPowerControl {
    fn sleep(&self) -> Result<()> { Self::run("suspend") }
    fn restart(&self) -> Result<()> { Self::run("reboot") }
    fn shutdown(&self) -> Result<()> { Self::run("poweroff") }
}
