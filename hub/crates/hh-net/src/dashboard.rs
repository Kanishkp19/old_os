//! Loopback-only dashboard on 127.0.0.1:47801 (TRD §2, API_SPEC §1).
//! Static bundle embedded in the binary; a per-start local session secret is
//! required on API calls (header `X-HH-Local`).

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;

use crate::AppState;

const INDEX_HTML: &str = include_str!("../../../dashboard/index.html");
const APP_JS: &str = include_str!("../../../dashboard/app.js");
const STYLE_CSS: &str = include_str!("../../../dashboard/style.css");

#[derive(Clone)]
pub struct DashboardState {
    pub app: AppState,
    pub token: String,
}

pub fn new_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Self-contained router (state baked in), ready for `axum::serve`.
pub fn router(app: AppState, token: String) -> Router {
    let state = DashboardState { app, token };
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(js))
        .route("/style.css", get(css))
        .route("/api/overview", get(overview))
        .route("/api/devices", get(devices))
        .route("/api/pair/open", post(pair_open))
        .route("/api/pair/qr", get(pair_qr))
        .route("/api/storage", get(storage))
        .route("/api/alerts", get(alerts))
        .route("/api/files", get(files))
        .route("/api/files/{id}/content", get(file_content_dash))
        .route("/api/files/{id}/thumb", get(file_thumb_dash))
        .route("/api/activity", get(activity))
        .route("/api/transfers", get(transfers))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/update", get(update))
        .route("/api/second-copy", get(second_copy_get).post(second_copy_set))
        .route("/api/similar", get(similar))
        .merge(crate::admin::router())
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), local_authorize))
        .with_state(state)
}

fn check_token(st: &DashboardState, headers: &HeaderMap) -> Result<(), StatusCode> {
    if let Some(h) = headers.get("x-hh-local").and_then(|v| v.to_str().ok()) {
        if h == st.token {
            return Ok(());
        }
    }
    if let Some(cookie) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("hh_local=") {
                if val == st.token {
                    return Ok(());
                }
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn index() -> impl IntoResponse {
    // Loading a public page grants no privileges. Authorized native clients
    // exchange their same-user secret at POST /api/session.
    Html(INDEX_HTML.replace("__HH_TOKEN__", ""))
}

fn allowed_host(value: &str) -> bool {
    matches!(value,"127.0.0.1:47801" | "localhost:47801")
}
fn allowed_origin(value: &str) -> bool {
    matches!(value,"http://127.0.0.1:47801" | "http://localhost:47801")
}
async fn local_authorize(State(st): State<DashboardState>, req: axum::extract::Request, next: axum::middleware::Next) -> Result<Response,StatusCode> {
    let headers = req.headers();
    if !headers.get("host").and_then(|v|v.to_str().ok()).is_some_and(allowed_host) { return Err(StatusCode::FORBIDDEN); }
    if headers.get("origin").and_then(|v|v.to_str().ok()).is_some_and(|o|!allowed_origin(o)) { return Err(StatusCode::FORBIDDEN); }
    if headers.get("sec-fetch-site").and_then(|v|v.to_str().ok()).is_some_and(|v|v == "cross-site") { return Err(StatusCode::FORBIDDEN); }
    if req.uri().path().starts_with("/api/") {
        check_token(&st,headers)?;
        if !matches!(*req.method(),axum::http::Method::GET | axum::http::Method::HEAD) {
            let explicit = headers.get("x-hh-local").and_then(|v|v.to_str().ok()).is_some_and(|v|v == st.token);
            let csrf = headers.get("x-hh-csrf").and_then(|v|v.to_str().ok()).is_some_and(|v|v == st.token);
            let origin = headers.get("origin").and_then(|v|v.to_str().ok()).is_some_and(allowed_origin);
            if !explicit && !(origin && csrf) { return Err(StatusCode::FORBIDDEN); }
        }
    }
    let mut response = next.run(req).await;
    response.headers_mut().insert("x-content-type-options",axum::http::HeaderValue::from_static("nosniff"));
    response.headers_mut().insert("referrer-policy",axum::http::HeaderValue::from_static("no-referrer"));
    if !req_path_is_attachment(&response) {
        response.headers_mut().insert("content-security-policy",axum::http::HeaderValue::from_static("default-src 'self'; img-src 'self' data: blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'none'"));
    }
    response.headers_mut().insert("cache-control",axum::http::HeaderValue::from_static("no-store"));
    Ok(response)
}
fn req_path_is_attachment(response: &Response) -> bool { response.headers().contains_key("content-disposition") }

async fn js() -> impl IntoResponse {
    ([("content-type", "text/javascript")], APP_JS)
}

async fn css() -> impl IntoResponse {
    ([("content-type", "text/css")], STYLE_CSS)
}

async fn overview(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let (hub_id, name) = st.app.db.hub_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let storage=st.app.storage.clone();
    let sum=tokio::task::spawn_blocking(move||storage.library_summary()).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let devices = st.app.db.list_devices().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active = st.app.db.list_transfers(None, Some("open")).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "hub_id": hub_id,
        "name": name,
        "version": hh_core::HUB_VERSION,
        "free_bytes": sum.free_bytes,
        "total_bytes": sum.total_bytes,
        "copies": sum.copies,
        "devices": devices.len(),
        "active_transfers": active.len(),
        "alerts": st.app.db.active_alert_count().unwrap_or(0),
        "pairing_open": st.app.pairing.is_open(),
    })))
}

async fn devices(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let devices = st.app.db.list_devices().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "items": devices.iter().map(|d| d.to_api()).collect::<Vec<_>>()
    })))
}

async fn pair_open(State(st): State<DashboardState>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    check_token(&st, &headers)?;
    st.app.pairing.open_window().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct QrQ {
    #[allow(dead_code)] // reserved for svg/png selection (UI_UX §4.2)
    format: Option<String>,
}

async fn pair_qr(State(st): State<DashboardState>, headers: HeaderMap, Query(_q): Query<QrQ>) -> Result<Response, StatusCode> {
    check_token(&st, &headers)?;
    let (hub_id, name) = st.app.db.hub_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let window = st.app.pairing.current_window().map_err(|_| StatusCode::CONFLICT)?;
    let fp = st.app.ca.fingerprint().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let addrs: Vec<String> = st
        .app
        .lan_addrs
        .read()
        .map(|a| a.clone())
        .unwrap_or_default();
    let payload = hh_auth::pairing::build_qr_payload(&hub_id, &window.token, &fp, &addrs, &name);
    let payload=format!("{payload}&fp_sha256={}",st.app.ca.full_fingerprint());
    let svg = hh_auth::pairing::qr_svg(&payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let secs = st.app.pairing.seconds_remaining();
    let code = window.manual_code.clone();
    let body = serde_json::json!({
        "payload": payload,
        "manual_code": code,
        "expires_in": secs,
    });
    Ok(Response::builder()
        .header("content-type", "application/json")
        .header("x-qr-svg", base64::engine::general_purpose::STANDARD.encode(svg))
        .body(body.to_string().into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
}

async fn storage(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let storage=st.app.storage.clone();
    let sum=tokio::task::spawn_blocking(move||storage.library_summary()).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let health = st.app.storage.health_summary().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "categories": sum.categories,
        "free_bytes": sum.free_bytes,
        "total_bytes": sum.total_bytes,
        "copies": sum.copies,
        "reclaimable_trash_bytes": sum.reclaimable_trash_bytes,
        "missing_files": sum.missing_files,
        "disks": health,
        "second_copy": st.app.storage.copy_coverage().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?,
        "second_copy_age_ms": st.app.storage.last_second_copy_age_ms().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?,
    })))
}

async fn alerts(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let c = st.app.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut stmt = c
        .prepare("SELECT id, severity, code, message, created_at FROM alerts WHERE resolved_at IS NULL AND acknowledged_at IS NULL ORDER BY created_at DESC")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "severity": r.get::<_, String>(1)?,
                "code": r.get::<_, String>(2)?,
                "message": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
            }))
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "items": items })))
}

#[derive(Deserialize)]
struct FilesQuery { category:Option<String>,q:Option<String>,cursor:Option<String>,sort:Option<String>,limit:Option<u32> }
async fn files(State(st): State<DashboardState>, headers: HeaderMap, Query(q):Query<FilesQuery>) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let page=st.app.storage.list_files_page(q.category.as_deref(),q.q.as_deref(),q.cursor.as_deref(),q.sort.as_deref(),q.limit.unwrap_or(100)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!(page)))
}

async fn file_content_dash(
    State(st): State<DashboardState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    check_token(&st, &headers)?;
    let meta = st.app.storage.get_file(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let path = st.app.storage.file_disk_path(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    crate::routes::stream_file(path,&meta,&headers).await.map_err(|e|StatusCode::from_u16(e.0.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
}

async fn file_thumb_dash(
    State(st): State<DashboardState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    check_token(&st, &headers)?;
    if let Ok(Some(path)) = st.app.photos.thumb_path(&id, 256) {
        if let Ok(bytes) = tokio::fs::read(&path).await {
            return Ok(Response::builder()
                .header("content-type", "image/jpeg")
                .header("cache-control", "public, max-age=86400")
                .body(Body::from(bytes))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);
        }
    }
    Err(StatusCode::NOT_FOUND)
}

// ---- W1.4: security activity ----

async fn activity(State(st): State<DashboardState>, headers: HeaderMap, Query(q): Query<ActivityQ>) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let (rows, total) = st
        .app
        .db
        .list_audit(q.device_id.as_deref(), q.limit.unwrap_or(50), q.offset.unwrap_or(0))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let names: std::collections::HashMap<String, String> = st
        .app
        .db
        .list_devices()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    Ok(Json(serde_json::json!({
        "total": total,
        "items": rows.iter().map(|r| serde_json::json!({
            "ts": r.ts,
            "device": r.device_id.as_ref().and_then(|d| names.get(d)).map(|s|s.as_str()).unwrap_or(r.device_id.as_deref().unwrap_or("system")),
            "action": r.action,
            "detail": r.detail,
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct ActivityQ {
    limit: Option<u32>,
    offset: Option<u32>,
    device_id: Option<String>,
}

// ---- W1.7: transfers with measured-rate ETA ----

async fn transfers(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let rows = st.app.db.list_transfers(None, None).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let names: std::collections::HashMap<String, String> = st
        .app
        .db
        .list_devices()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    let now = hh_core::time::now_ms();
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|t| {
            // Measured-rate ETA: average rate since the transfer started.
            // Honest: transfers slower than 1 KB/s get no ETA.
            let (rate_bps, eta_secs) = if t.status == "open" && t.bytes_verified > 0 {
                let elapsed = (now - t.created_at).max(1) as f64 / 1000.0;
                let rate = t.bytes_verified as f64 / elapsed;
                if rate > 1024.0 && t.size > t.bytes_verified {
                    (rate, Some(((t.size - t.bytes_verified) as f64 / rate) as i64))
                } else if t.size <= t.bytes_verified {
                    (rate, Some(0))
                } else {
                    (rate, None)
                }
            } else {
                (0.0, None)
            };
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "device": names.get(&t.device_id).cloned().unwrap_or_else(|| t.device_id.clone()),
                "kind": t.kind,
                "status": t.status,
                "size": t.size,
                "bytes_verified": t.bytes_verified,
                "rate_bps": rate_bps,
                "eta_secs": eta_secs,
                "error_code": t.error_code,
                "updated_at": t.updated_at,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "items": items })))
}

// ---- W1.5: diagnostics export (opt-in, privacy-scoped) ----

async fn diagnostics(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    check_token(&st, &headers)?;
    let app = st.app.clone();
    let blob = tokio::task::spawn_blocking(move || hh_service_diagnostics(&app))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ts = hh_core::time::now_ms();
    Ok(Response::builder()
        .header("content-type", "application/json")
        .header(
            "content-disposition",
            format!("attachment; filename=\"homehub-diagnostics-{ts}.json\""),
        )
        .body(Body::from(blob.to_string()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
}

/// Privacy-scoped diagnostics (TRD §13, AGENTS.md §2.7): versions, counters,
/// worker liveness, audit actions and the last log lines — with the library
/// root scrubbed so no file names or file contents ever leave the machine.
fn hh_service_diagnostics(app: &AppState) -> serde_json::Value {
    let uptime_ms = hh_core::time::now_ms() - app.started_at_ms;
    let devices = app.db.list_devices().map(|d| d.len()).unwrap_or(0);
    let open_transfers = app.db.list_transfers(None, Some("open")).map(|t| t.len()).unwrap_or(0);
    let total_transfers = app.db.list_transfers(None, None).map(|t| t.len()).unwrap_or(0);
    let (_, audit_total) = app.db.list_audit(None, 1, 0).unwrap_or((vec![], 0));
    let last_audit = app
        .hw
        .latest_audit()
        .ok()
        .flatten()
        .and_then(|v| v.get("ratings").cloned())
        .unwrap_or(serde_json::Value::Null);
    // Export bounded counters and action names only. Raw log details may
    // contain filenames or peer-provided metadata and are never exported.
    let log_tail:Vec<String>=Vec::new();
    serde_json::json!({
        "generated_at": hh_core::time::now_ms(),
        "privacy_note": "Contains no file names or file contents; library and data paths are scrubbed from logs.",
        "hub": {
            "version": hh_core::HUB_VERSION,
            "hub_id": app.db.hub_identity().map(|(i, _)| i).unwrap_or_default(),
            "uptime_ms": uptime_ms,
            "features": app.cfg.features,
        },
        "counters": {
            "devices": devices,
            "transfers_open": open_transfers,
            "transfers_total": total_transfers,
            "audit_events": audit_total,
            "active_alerts": app.db.active_alert_count().unwrap_or(0),
        },
        "hardware_ratings": last_audit,
        "second_copy_age_ms": app.storage.last_second_copy_age_ms().ok().flatten(),
        "log_tail": log_tail,
    })
}

// ---- W2.6: update channel for the tray/dashboard ----

async fn update(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let db = st.app.db.clone();
    let check = tokio::task::spawn_blocking(move || crate::update::check_from_settings(&db, hh_core::HUB_VERSION))
        .await
        .unwrap_or(crate::update::UpdateCheck {
            available: false,
            current_version: hh_core::HUB_VERSION.into(),
            latest_version: None,
            url: None,
            sha256: None,
            reason: Some("check_failed".into()),
        });
    Ok(Json(serde_json::json!({
        "available": check.available,
        "current_version": check.current_version,
        "latest_version": check.latest_version,
        "url": check.url,
        "reason": check.reason,
    })))
}

// ---- W2.1: second-copy schedule settings ----

#[derive(Deserialize)]
struct SecondCopySet {
    interval_hours: u32,
}

async fn second_copy_get(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let interval: u32 = st
        .app
        .db
        .get_setting("second_copy.interval_minutes")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1440);
    Ok(Json(serde_json::json!({
        "interval_hours": interval / 60,
        "interval_minutes": interval,
        "last_run_age_ms": st.app.storage.last_second_copy_age_ms().ok().flatten(),
        "coverage": st.app.storage.copy_coverage().ok(),
    })))
}

async fn second_copy_set(State(st): State<DashboardState>, headers: HeaderMap, Json(req): Json<SecondCopySet>) -> Result<StatusCode, StatusCode> {
    check_token(&st, &headers)?;
    if !(1..=720).contains(&req.interval_hours) { return Err(StatusCode::BAD_REQUEST); }
    st.app
        .db
        .set_setting("second_copy.interval_minutes", &(req.interval_hours * 60).to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = st.app.db.audit(None, "second_copy_schedule", Some(&req.interval_hours.to_string()), None);
    Ok(StatusCode::NO_CONTENT)
}

// ---- W2.3: similar photos for the dashboard ----

async fn similar(State(st): State<DashboardState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    check_token(&st, &headers)?;
    let groups = st.app.storage.list_similar().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out: Vec<serde_json::Value> = vec![];
    for g in groups {
        let mut members: Vec<serde_json::Value> = vec![];
        for fid in &g.file_ids {
            let meta = st.app.storage.get_file(fid).ok();
            members.push(serde_json::json!({
                "file_id": fid,
                "name": meta.as_ref().map(|m| m.name.clone()).unwrap_or_default(),
                "size": meta.as_ref().map(|m| m.size).unwrap_or(0),
            }));
        }
        out.push(serde_json::json!({
            "id": g.id,
            "reclaimable_bytes": g.reclaimable_bytes,
            "members": members,
        }));
    }
    Ok(Json(serde_json::json!({ "items": out })))
}
