//! Windows platform implementations for the session helper (TRD §9).
//!
//! Everything here is exercised only on a real Windows user session; it is
//! compile-checked on Windows CI. API notes are kept next to each call so a
//! version bump of the `windows` crate is a mechanical fix.

// ---- input injection (FR-7.1/7.2) ----

pub fn mouse_move(dx: i32, dy: i32) -> Result<(), String> {
    send_mouse(MOUSEEVENTF_MOVE, dx, dy, 0)
}

pub fn click(button: &str, count: u8) -> Result<(), String> {
    let (down, up) = match button {
        "left" => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        other => return Err(format!("unknown button {other:?}")),
    };
    for _ in 0..count.clamp(1, 2) {
        send_mouse(down, 0, 0, 0)?;
        send_mouse(up, 0, 0, 0)?;
    }
    Ok(())
}

pub fn scroll(dy: i32) -> Result<(), String> {
    // Positive wheel delta scrolls up on Windows; clients send positive dy up.
    send_mouse(MOUSEEVENTF_WHEEL, 0, 0, (dy * 120) as u32)
}

fn send_mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: u32) -> Result<(), String> {
    let mut inp = INPUT::default();
    inp.r#type = INPUT_MOUSE;
    inp.Anonymous.mi = MOUSEINPUT {
        dx,
        dy,
        mouseData: data,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };
    // SAFETY: plain SendInput call with a well-formed INPUT record.
    let sent = unsafe { SendInput(&[inp], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        return Err("SendInput failed (blocked by UIPI or secure desktop?)".into());
    }
    Ok(())
}

fn vk_for_key(key: &str) -> Option<u16> {
    let k = key.to_lowercase();
    let named = match k.as_str() {
        "enter" | "return" => 0x0D,
        "backspace" => 0x08,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "space" => 0x20,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "delete" | "del" => 0x2E,
        "insert" => 0x2D,
        "shift" => 0x10,
        "ctrl" | "control" => 0x11,
        "alt" => 0x12,
        "meta" | "win" => 0x5B,
        "menu" => 0x5D,
        _ => return char_key(&k),
    };
    Some(named)
}

fn char_key(k: &str) -> Option<u16> {
    let mut chars = k.chars();
    let (c, rest) = (chars.next()?, chars.next().is_none());
    if !rest {
        let u = c.to_ascii_uppercase();
        if u.is_ascii_alphabetic() {
            return Some(u as u16);
        }
        if u.is_ascii_digit() {
            return Some(u as u16);
        }
    }
    if let Some(f) = k.strip_prefix('f') {
        if let Ok(n) = f.parse::<u16>() {
            if (1..=12).contains(&n) {
                return Some(0x6F + n); // VK_F1 = 0x70
            }
        }
    }
    None
}

pub fn key(key: &str, down: bool) -> Result<(), String> {
    let vk = vk_for_key(key).ok_or_else(|| format!("unsupported key {key:?}"))?;
    send_key(vk, down)
}

fn send_key(vk: u16, down: bool) -> Result<(), String> {
    let mut inp = INPUT::default();
    inp.r#type = INPUT_KEYBOARD;
    inp.Anonymous.ki = KEYBDINPUT {
        wVk: VIRTUAL_KEY(vk),
        wScan: 0,
        dwFlags: if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP },
        time: 0,
        dwExtraInfo: 0,
    };
    // SAFETY: plain SendInput call with a well-formed INPUT record.
    let sent = unsafe { SendInput(&[inp], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        return Err("SendInput failed".into());
    }
    Ok(())
}

pub fn text(s: &str) -> Result<(), String> {
    for c in s.chars() {
        let code = u32::from(c);
        let scan = u16::try_from(code).map_err(|_| "char outside BMP".to_string())?;
        for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
            let mut inp = INPUT::default();
            inp.r#type = INPUT_KEYBOARD;
            inp.Anonymous.ki = KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            };
            // SAFETY: plain SendInput call with a well-formed INPUT record.
            let sent = unsafe { SendInput(&[inp], std::mem::size_of::<INPUT>() as i32) };
            if sent != 1 {
                return Err("SendInput failed".into());
            }
        }
    }
    Ok(())
}

pub fn media_key(key: &str) -> Result<(), String> {
    let vk: u16 = match key {
        "play_pause" => 0xB3,
        "next" => 0xB0,
        "prev" => 0xB1,
        "vol_up" => 0xAF,
        "vol_down" => 0xAE,
        "mute" => 0xAD,
        other => return Err(format!("unknown media key {other:?}")),
    };
    send_key(vk, true)?;
    send_key(vk, false)
}

// ---- power (FR-7.3) ----

pub fn power(action: &str) -> Result<(), String> {
    match action {
        "sleep" => {
            // SAFETY: documented Win32 power call.
            let ok = unsafe { SetSuspendState(false, false, false) };
            ok.map_err(|e| format!("SetSuspendState: {e}"))
        }
        "restart" | "shutdown" => {
            ensure_shutdown_privilege()?;
            let flags = match action {
                "restart" => EWX_REBOOT | EWX_FORCEIFHUNG,
                _ => EWX_SHUTDOWN | EWX_FORCEIFHUNG,
            };
            // SAFETY: documented shutdown call; privilege adjusted above.
            let ok = unsafe { ExitWindowsEx(flags, SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER) };
            ok.map_err(|e| format!("ExitWindowsEx: {e}"))
        }
        other => Err(format!("unknown power action {other:?}")),
    }
}

fn ensure_shutdown_privilege() -> Result<(), String> {
    // SAFETY: standard token-privilege dance (OpenProcessToken →
    // LookupPrivilegeValueW → AdjustTokenPrivileges); handles closed after.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token)
            .map_err(|e| format!("OpenProcessToken: {e}"))?;
        let mut luid = LUID::default();
        let name = windows::core::w!("SeShutdownPrivilege");
        LookupPrivilegeValueW(None, name, &mut luid).map_err(|e| format!("LookupPrivilegeValueW: {e}"))?;
        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let res = AdjustTokenPrivileges(token, false, Some(&mut tp), 0, None, None);
        let _ = CloseHandle(token);
        // ERROR_NOT_ALL_ASSIGNED means the privilege was missing — usually
        // because the helper was not started by the installer's user.
        match res {
            Ok(()) => Ok(()),
            Err(e) => Err(format!("AdjustTokenPrivileges: {e}")),
        }
    }
}

// ---- system info via WMI / netsh (M3 hardware audit) ----

pub fn system_info() -> Result<serde_json::Value, String> {
    let battery_health_pct = wmi_battery_health();
    let has_camera = wmi_has_camera();
    let wifi_standard = netsh_wifi_standard();

    Ok(serde_json::json!({
        "battery_health_pct": battery_health_pct,
        "has_camera": has_camera,
        "wifi_standard": wifi_standard,
    }))
}

fn wmi_battery_health() -> Option<u32> {
    #[derive(serde::Deserialize)]
    #[allow(non_snake_case, dead_code)]
    struct BatteryStaticData {
        DesignCapacity: Option<u32>,
        FullChargedCapacity: Option<u32>,
    }
    let conn = wmi_connection()?;
    let rows: Vec<BatteryStaticData> = conn
        .raw_query("SELECT DesignCapacity, FullChargedCapacity FROM BatteryStaticData")
        .ok()?;
    let row = rows.first()?;
    let design = row.DesignCapacity? as f64;
    let full = row.FullChargedCapacity? as f64;
    if design <= 0.0 {
        return None;
    }
    Some(((full / design) * 100.0).round().clamp(0.0, 100.0) as u32)
}

fn wmi_has_camera() -> bool {
    #[derive(serde::Deserialize)]
    #[allow(non_snake_case, dead_code)]
    struct PnPEntity {
        #[serde(rename = "PNPClass")]
        pnp_class: Option<String>,
    }
    let Some(conn) = wmi_connection() else { return false };
    let rows: Vec<PnPEntity> = conn
        .raw_query("SELECT PNPClass FROM Win32_PnPEntity WHERE PNPClass='Camera' OR PNPClass='Image'")
        .unwrap_or_default();
    !rows.is_empty()
}

fn wmi_connection() -> Option<wmi::WMIConnection> {
    // The wmi crate initializes COM on construction of COMLibrary.
    wmi::COMLibrary::new().ok().and_then(|c| wmi::WMIConnection::new(c).ok())
}

fn netsh_wifi_standard() -> Option<String> {
    let out = std::process::Command::new("netsh")
        .args(["wlan", "show", "drivers"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let line = text.lines().find(|l| l.contains("radio types supported"))?;
    let standard = ["802.11ax", "802.11ac", "802.11n", "802.11g"]
        .into_iter()
        .find(|s| line.contains(s))?;
    Some(standard.to_string())
}

// ---- hotspot (M3, FR-8.3) — WinRT tethering ----

pub fn hotspot(enable: bool, ssid: Option<&str>, passphrase: Option<&str>) -> Result<serde_json::Value, String> {
    use windows::Networking::Connectivity::NetworkOperatorTetheringManager;

    // WinRT async ops are driven to completion synchronously here; the helper
    // call is already off the service's async runtime. Typical latency < 2 s.
    let mgr = NetworkOperatorTetheringManager::GetForCurrentProfile()
        .map_err(|e| format!("tethering unavailable: {e}"))?;

    if let (Some(s), Some(p)) = (ssid, passphrase) {
        if let Ok(cfg) = mgr.GetCurrentAccessPointConfiguration() {
            let _ = cfg.SetSsid(&windows::core::HSTRING::from(s));
            let _ = cfg.SetPassphrase(&windows::core::HSTRING::from(p));
            mgr.ConfigureAccessPointAsync(&cfg)
                .and_then(|op| op.get())
                .map_err(|e| format!("ConfigureAccessPoint: {e}"))?;
        }
    }

    let op = if enable {
        mgr.StartTetheringAsync().map_err(|e| format!("StartTethering: {e}"))?
    } else {
        mgr.StopTetheringAsync().map_err(|e| format!("StopTethering: {e}"))?
    };
    op.get().map_err(|e| format!("tethering op: {e}"))?;

    let state = mgr
        .TetheringOperationalState()
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|_| "Unknown".into());
    Ok(serde_json::json!({ "state": state }))
}

// ---- BitLocker detect/recommend (FR-4.5) ----

pub fn bitlocker_status() -> Result<serde_json::Value, String> {
    let out = std::process::Command::new("manage-bde")
        .args(["-status", "C:"])
        .output()
        .map_err(|e| format!("manage-bde unavailable: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let protected = text.contains("protection status: protection on")
        || text.contains("schutzstatus: aktiviert");
    // Requires admin to run manage-bde -status on some systems; report
    // honestly instead of pretending it is off.
    let readable = out.status.success();
    Ok(serde_json::json!({
        "available": readable,
        "protected": if readable { Some(protected) } else { None },
        "recommendation": if readable && !protected {
            "Turn on BitLocker (Settings → Privacy & security → Device encryption) to protect the library at rest."
        } else {
            ""
        },
    }))
}

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{
    AdjustTokenPrivileges, CloseHandle, LookupPrivilegeValueW, LUID, LUID_AND_ATTRIBUTES,
    SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, EWX_FORCEIFHUNG, EWX_REBOOT, EWX_SHUTDOWN, SHTDN_REASON_MAJOR_OTHER,
    SHTDN_REASON_MINOR_OTHER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY, MOUSE_EVENT_FLAGS,
};
