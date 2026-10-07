//! Request dispatch: maps protocol ops onto platform implementations.
//!
//! Windows implementations live in `platform_win`; everywhere else the same
//! ops answer with the honest "unsupported" error so the protocol surface is
//! identical and testable cross-platform.

use hh_remote::helper_protocol::{HelperRequest, HelperResponse};

use crate::{err, ok_data};

pub fn dispatch(req: &HelperRequest) -> HelperResponse {
    match req {
        HelperRequest::Ping => ok_data(serde_json::json!({
            "helper": env!("CARGO_PKG_VERSION"),
            "platform": std::env::consts::OS,
        })),
        HelperRequest::MouseMove { dx, dy } => wrap(platform::mouse_move(*dx, *dy)),
        HelperRequest::Click { button, count } => wrap(platform::click(button, *count)),
        HelperRequest::Scroll { dy } => wrap(platform::scroll(*dy)),
        HelperRequest::Key { key, down } => wrap(platform::key(key, *down)),
        HelperRequest::Text { s } => wrap(platform::text(s)),
        HelperRequest::Media { key } => wrap(platform::media_key(key)),
        HelperRequest::Power { action } => wrap(platform::power(action)),
        HelperRequest::SystemInfo => match platform::system_info() {
            Ok(v) => ok_data(v),
            Err(e) => err(e),
        },
        HelperRequest::Hotspot { enable, ssid, passphrase } => {
            match platform::hotspot(*enable, ssid.as_deref(), passphrase.as_deref()) {
                Ok(v) => ok_data(v),
                Err(e) => err(e),
            }
        }
        HelperRequest::BitlockerStatus => match platform::bitlocker_status() {
            Ok(v) => ok_data(v),
            Err(e) => err(e),
        },
        #[cfg(feature = "screen")]
        HelperRequest::ScreenOffer { sdp, preset, kind } => {
            match crate::screen::handle_offer(sdp, preset, kind) {
                Ok(v) => ok_data(v),
                Err(e) => err(e),
            }
        }
        #[cfg(feature = "screen")]
        HelperRequest::AddIce { candidate } => {
            match crate::screen::add_ice(candidate) {
                Ok(()) => ok_data(serde_json::json!({})),
                Err(e) => err(e),
            }
        }
        #[cfg(feature = "screen")]
        HelperRequest::ScreenStop => match crate::screen::stop() {
            Ok(()) => ok_data(serde_json::json!({})),
            Err(e) => err(e),
        },
        #[cfg(not(feature = "screen"))]
        HelperRequest::ScreenOffer { .. }
        | HelperRequest::AddIce { .. }
        | HelperRequest::ScreenStop => err("screen host not compiled into this helper build"),
    }
}

fn wrap(r: Result<(), String>) -> HelperResponse {
    match r {
        Ok(()) => ok_data(serde_json::json!({})),
        Err(e) => err(e),
    }
}

/// Platform surface used by dispatch. Windows fills these from
/// `platform_win`; other platforms return honest unsupported errors.
pub mod platform {
    #[cfg(windows)]
    pub use crate::platform_win::*;

    #[cfg(not(windows))]
    pub fn mouse_move(_dx: i32, _dy: i32) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn click(_button: &str, _count: u8) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn scroll(_dy: i32) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn key(_key: &str, _down: bool) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn text(_s: &str) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn media_key(_key: &str) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn power(_action: &str) -> Result<(), String> {
        unsupported()
    }
    #[cfg(not(windows))]
    pub fn system_info() -> Result<serde_json::Value, String> {
        unsupported_value()
    }
    #[cfg(not(windows))]
    pub fn hotspot(_enable: bool, _ssid: Option<&str>, _pass: Option<&str>) -> Result<serde_json::Value, String> {
        unsupported_value()
    }
    #[cfg(not(windows))]
    pub fn bitlocker_status() -> Result<serde_json::Value, String> {
        unsupported_value()
    }

    #[cfg(not(windows))]
    fn unsupported() -> Result<(), String> {
        Err("not supported on this platform (Windows helper required)".into())
    }

    #[cfg(not(windows))]
    fn unsupported_value() -> Result<serde_json::Value, String> {
        Err("not supported on this platform (Windows helper required)".into())
    }
}
