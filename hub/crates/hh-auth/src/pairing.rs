//! Pairing window, one-time tokens, manual codes (TRD §5, SECURITY §5).
//!
//! Rules enforced here:
//! - Window opens only on explicit user action; closes after success or 5 min.
//! - Token: 128-bit random, stored SHA-256-hashed, single use, 5-attempt lockout.
//! - Manual code: 6 digits, 2-min TTL, 3 attempts, requires on-Hub confirmation.

use std::sync::Mutex;

use base64::Engine;
use hh_core::error::{Error, Result};
use hh_core::time::now_ms;
use hh_core::{PAIR_CODE_TTL_MS, PAIR_TOKEN_TTL_MS};
use hh_db::Db;
use rand::RngCore;
use sha2::{Digest, Sha256};

const MAX_TOKEN_ATTEMPTS: i64 = 5;
const MAX_CODE_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone)]
pub struct PairingWindow {
    pub token: String, // raw, shown only in the QR payload — never stored
    pub token_hash: String,
    pub manual_code: String,
    pub expires_at: i64,
    pub code_expires_at: i64,
    /// Failed 6-digit-code attempts this window (low entropy → hard cap).
    code_attempts: u32,
}

pub struct PairingManager {
    db: Db,
    window: Mutex<Option<PairingWindow>>,
}

impl PairingManager {
    pub fn new(db: Db) -> Self {
        Self { db, window: Mutex::new(None) }
    }

    /// Open a new pairing window (user clicked "Pair a device").
    pub fn open_window(&self) -> Result<PairingWindow> {
        let mut token_bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut token_bytes);
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token_bytes);
        let token_hash = hex_sha256(token.as_bytes());

        let mut code_bytes = [0u8; 4];
        rand::rngs::OsRng.fill_bytes(&mut code_bytes);
        let code = format!("{:06}", u32::from_be_bytes(code_bytes) % 1_000_000);

        let now = now_ms();
        let window = PairingWindow {
            token: token.clone(),
            token_hash: token_hash.clone(),
            manual_code: code,
            expires_at: now + PAIR_TOKEN_TTL_MS,
            code_expires_at: now + PAIR_CODE_TTL_MS,
            code_attempts: 0,
        };
        self.db
            .insert_pairing_token(&ulid::Ulid::new().to_string(), &token_hash, window.expires_at)?;
        *self.window.lock().map_err(|_| Error::Internal("pairing mutex poisoned".into()))? =
            Some(window.clone());
        self.db.audit(None, "pair_window_open", None, None)?;
        Ok(window)
    }

    pub fn close_window(&self) {
        if let Ok(mut w) = self.window.lock() {
            *w = None;
        }
    }

    pub fn is_open(&self) -> bool {
        self.window
            .lock()
            .map(|w| w.as_ref().map(|w| w.expires_at > now_ms()).unwrap_or(false))
            .unwrap_or(false)
    }

    /// Seconds remaining on the current window (for the UI countdown).
    pub fn seconds_remaining(&self) -> i64 {
        self.window
            .lock()
            .ok()
            .and_then(|w| w.as_ref().map(|w| (w.expires_at - now_ms()) / 1000))
            .unwrap_or(0)
            .max(0)
    }

    /// Validate a QR token. Burns the token on success (single use).
    pub fn validate_token(&self, presented: &str) -> Result<String /* token row id */> {
        if !self.is_open() {
            return Err(Error::PairingClosed);
        }
        let hash = hex_sha256(presented.as_bytes());
        let (id, expires_at, used_at, attempts) = self
            .db
            .get_pairing_token(&hash)?
            .ok_or(Error::InvalidToken)?;
        if attempts >= MAX_TOKEN_ATTEMPTS {
            return Err(Error::PairingLocked);
        }
        if used_at.is_some() {
            return Err(Error::InvalidToken); // PA-03: reused token rejected
        }
        if expires_at < now_ms() {
            return Err(Error::TokenExpired);
        }
        Ok(id)
    }

    /// Validate a manual 6-digit code (low entropy: 3 attempts, and the caller
    /// MUST additionally require on-Hub confirmation — API_SPEC §3).
    pub fn validate_code(&self, presented: &str) -> Result<()> {
        let mut window = self
            .window
            .lock()
            .map_err(|_| Error::Internal("pairing mutex poisoned".into()))?;
        let w = window.as_mut().ok_or(Error::PairingClosed)?;
        if w.code_expires_at < now_ms() {
            return Err(Error::TokenExpired);
        }
        if presented.trim() == w.manual_code {
            return Ok(());
        }
        // 6-digit codes are low entropy: close the window after 3 bad tries.
        w.code_attempts += 1;
        if w.code_attempts >= MAX_CODE_ATTEMPTS {
            *window = None;
            return Err(Error::PairingLocked);
        }
        Err(Error::InvalidToken)
    }

    pub fn burn(&self, token_row_id: &str, device_id: &str) -> Result<()> {
        self.db.burn_token(token_row_id, device_id)?;
        self.close_window();
        Ok(())
    }
}

/// QR payload per API_SPEC §3:
/// `homehub://pair?h=<hub_id>&t=<token>&fp=<ca_fp>&a=<ip:port,...>&n=<name>`.
pub fn build_qr_payload(
    hub_id: &str,
    token: &str,
    ca_fingerprint: &str,
    addrs: &[String],
    name: &str,
) -> String {
    let a = addrs.join(",");
    format!(
        "homehub://pair?h={}&t={}&fp={}&a={}&n={}",
        urlenc(hub_id),
        urlenc(token),
        urlenc(ca_fingerprint),
        urlenc(&a),
        urlenc(name),
    )
}

/// Render the QR payload as an SVG for the dashboard/tray pairing screen.
pub fn qr_svg(payload: &str) -> Result<String> {
    let code = qrcode::QrCode::new(payload.as_bytes())
        .map_err(|e| Error::Internal(format!("qr encode: {e}")))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(qrcode::render::svg::Color("#1A1D1B")) // --ink
        .light_color(qrcode::render::svg::Color("#FFFFFF")) // --surface
        .build();
    Ok(svg)
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn urlenc(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> PairingManager {
        PairingManager::new(Db::open_memory().unwrap())
    }

    #[test]
    fn token_lifecycle() {
        let m = mgr();
        // burn_token FKs used_by_device_id → devices(id); insert the device first.
        m.db
            .insert_device(
                &hh_db::DeviceRow {
                    id: "dev1".into(),
                    name: "Test Phone".into(),
                    platform: "android".into(),
                    model: None,
                    app_version: None,
                    cert_serial: "serial-1".into(),
                    cert_expires_at: 0,
                    scopes: vec![],
                    paired_at: 0,
                    last_seen_at: None,
                    status: "active".into(),
                },
                "PEM",
                "PUB",
            )
            .unwrap();
        let w = m.open_window().unwrap();
        assert!(m.is_open());
        let row = m.validate_token(&w.token).unwrap();
        m.burn(&row, "dev1").unwrap();
        assert!(!m.is_open(), "window closes after success");
        assert!(matches!(m.validate_token(&w.token), Err(Error::PairingClosed)));
    }

    #[test]
    fn wrong_token_rejected() {
        let m = mgr();
        m.open_window().unwrap();
        assert!(matches!(m.validate_token("wrong"), Err(Error::InvalidToken)));
    }

    #[test]
    fn manual_code() {
        let m = mgr();
        let w = m.open_window().unwrap();
        assert!(m.validate_code("000001").is_err() || w.manual_code == "000001");
        assert!(m.validate_code(&w.manual_code).is_ok());
    }

    #[test]
    fn qr_payload_format() {
        let p = build_qr_payload("01J", "tok", "abcdef0123456789", &["192.168.1.5:47802".into()], "My Hub");
        assert!(p.starts_with("homehub://pair?h=01J&t=tok&fp=abcdef0123456789"));
        assert!(p.contains("192.168.1.5%3A47802"));
    }

    #[test]
    fn qr_svg_renders() {
        assert!(qr_svg("homehub://pair?h=x&t=y&fp=z").unwrap().contains("<svg"));
    }
}
