//! TLS pairing: bootstrap exposes only the public CA; token submission must
//! use a connection verified against the QR-pinned CA.
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use hh_core::error::Error;
use hh_core::types::{PairHubInfo, PairRequest, PairResponse};
use hh_db::DeviceRow;
use serde_json::json;
use crate::routes::ApiErr;
use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new().route("/pair/ca", get(ca)).route("/pair", post(pair))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)).with_state(state)
}
async fn ca(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({"ca_cert_pem": s.ca.ca_cert_pem}))
}
async fn pair(State(s): State<AppState>, Json(req): Json<PairRequest>) -> Result<Json<PairResponse>, ApiErr> {
    if s.db.get_setting("sharing.paused")?.as_deref() == Some("true") {
        return Err(Error::StorageUnavailable("sharing paused".into()).into());
    }
    let name = hh_core::paths::sanitize_component(&req.device_name)?;
    if !["android","ios","macos","windows","linux","web"].contains(&req.platform.as_str()) {
        return Err(Error::BadRequest("unsupported platform".into()).into());
    }
    if req.csr_pem.len() > 64 * 1024 || req.model.as_ref().is_some_and(|v| v.len() > 200) || req.app_version.as_ref().is_some_and(|v| v.len() > 100) {
        return Err(Error::TooLarge("pairing identity".into()).into());
    }
    let confirm = s.db.get_setting("pair.confirm_on_hub")?.as_deref() == Some("true");
    let result = s.pairing.claim(&req, confirm, |token_id| {
        let device_id = ulid::Ulid::new().to_string();
        let (cert_pem, serial, expires) = s.ca.issue_device_cert(&req.csr_pem, &device_id)?;
        let mut scopes = vec!["files".to_owned(), "transfer".to_owned(), "photos".to_owned()];
        if s.db.list_devices()?.is_empty() { scopes.push("admin".to_owned()); }
        let d = DeviceRow { id: device_id.clone(), name: name.clone(), platform: req.platform.clone(), model: req.model.clone(), app_version: req.app_version.clone(), cert_serial: serial, cert_expires_at: expires, scopes: scopes.clone(), paired_at: hh_core::time::now_ms(), last_seen_at: None, status: "active".into() };
        s.db.insert_paired_device(&d, &cert_pem, token_id)?;
        let (hub_id, hub_name) = s.db.hub_identity()?;
        Ok(PairResponse { device_id, cert_pem, ca_cert_pem: s.ca.ca_cert_pem.clone(), cert_expires_at: expires, scopes, hub: PairHubInfo { id: hub_id, name: hub_name, api: hh_core::API_VERSION } })
    })?;
    s.db.audit(Some(&result.device_id), "pair", Some(&req.platform), None)?;
    s.events.emit("device.paired", json!({"device_id": result.device_id, "name": name}));
    Ok(Json(result))
}
