//! Pairing endpoint on port 47802 (TLS, no client cert — API_SPEC §3).
//! Token-gated and logically time-boxed: when no pairing window is open the
//! handler returns 401 PAIRING_CLOSED. The port only accepts LAN sources.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use hh_core::error::Error;
use hh_core::types::{PairHubInfo, PairRequest, PairResponse};
use hh_db::DeviceRow;
use serde::Serialize;

use crate::routes::ApiErr;
use crate::AppState;

#[derive(Serialize)]
struct PairStatus {
    open: bool,
    seconds_remaining: i64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/pair", post(pair))
        .route("/pair/status", get(pair_status))
        .with_state(state)
}

async fn pair_status(State(s): State<AppState>) -> Json<PairStatus> {
    Json(PairStatus {
        open: s.pairing.is_open(),
        seconds_remaining: s.pairing.seconds_remaining(),
    })
}

async fn pair(
    State(s): State<AppState>,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiErr> {
    if !s.pairing.is_open() {
        return Err(ApiErr(Error::PairingClosed));
    }
    if !matches!(req.platform.as_str(), "android" | "ios" | "macos" | "windows" | "linux" | "web") {
        return Err(ApiErr(Error::BadRequest("unknown platform".into())));
    }

    // Token (QR) or manual code. Manual code requires on-Hub confirmation
    // (API_SPEC §3): here we require the dashboard confirm flag setting.
    let token_row = if let Some(token) = &req.token {
        s.pairing.validate_token(token)?
    } else if let Some(code) = &req.code {
        s.pairing.validate_code(code)?;
        // Low-entropy codes require confirmation on the Hub itself. The
        // dashboard shows the prompt; pairing proceeds only if the setting
        // `pair.confirm_on_hub` is off or the dashboard confirmed (the
        // dashboard route flips `pair.confirmed` after user clicks Allow).
        let needs_confirm = s
            .db
            .get_setting("pair.confirm_on_hub")?
            .map(|v| v != "false")
            .unwrap_or(true);
        if needs_confirm {
            let confirmed = s
                .db
                .get_setting("pair.confirmed")?
                .map(|v| v == "true")
                .unwrap_or(false);
            if !confirmed {
                return Err(ApiErr(Error::PairingLocked)); // dashboard prompts; client retries
            }
            let _ = s.db.set_setting("pair.confirmed", "false");
        }
        String::new() // code path: no token row to burn
    } else {
        return Err(ApiErr(Error::BadRequest("token or code required".into())));
    };

    let device_id = ulid::Ulid::new().to_string();
    let (cert_pem, serial, expires_at) = s.ca.issue_device_cert(&req.csr_pem, &device_id)?;
    let scopes = default_scopes_for(&req.platform, s.db.list_devices()?.is_empty());
    let name = hh_core::paths::sanitize_component(&req.device_name)?;

    s.db.insert_device(
        &DeviceRow {
            id: device_id.clone(),
            name: name.clone(),
            platform: req.platform.clone(),
            model: req.model.clone(),
            app_version: req.app_version.clone(),
            cert_serial: serial,
            cert_expires_at: expires_at,
            scopes: scopes.clone(),
            paired_at: hh_core::time::now_ms(),
            last_seen_at: None,
            status: "active".into(),
        },
        &cert_pem,
        "", // public key retained inside cert_pem
    )?;
    if !token_row.is_empty() {
        s.pairing.burn(&token_row, &device_id)?;
    }
    s.pairing.close_window();
    s.db.audit(Some(&device_id), "pair", Some(&req.platform), None)?;

    let (hub_id, hub_name) = s.db.hub_identity()?;
    s.events.emit("device.paired", serde_json::json!({"device_id": device_id, "name": name}));
    Ok(Json(PairResponse {
        device_id,
        cert_pem,
        ca_cert_pem: s.ca.ca_cert_pem.clone(),
        cert_expires_at: expires_at,
        scopes,
        hub: PairHubInfo { id: hub_id, name: hub_name, api: hh_core::API_VERSION },
    }))
}

/// First paired device becomes the owner with `admin` (SECURITY §6).
fn default_scopes_for(_platform: &str, is_first_device: bool) -> Vec<String> {
    let mut v = vec!["files".to_string(), "transfer".to_string(), "photos".to_string()];
    if is_first_device {
        v.push("admin".to_string());
    }
    v
}
