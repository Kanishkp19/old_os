//! API routes (API_SPEC v1). Default-deny: every handler resolves a
//! PeerIdentity (from the mTLS handshake) and checks its scope.

use axum::body::Body;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::Response;
use futures::SinkExt;
use axum::routing::{delete, get, patch, post, put};
use axum::{Extension, Json, Router};
use hh_core::error::Error;
use hh_core::types::*;
use hh_core::{API_VERSION, HUB_VERSION};
use serde::Deserialize;

use crate::AppState;

type ApiResult<T> = Result<T, ApiErr>;

/// Error wrapper that renders API_SPEC §1 JSON.
pub struct ApiErr(pub Error);

impl From<Error> for ApiErr {
    fn from(e: Error) -> Self {
        ApiErr(e)
    }
}

impl axum::response::IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        // SECURITY §10: internal errors never leak paths/details.
        let msg = if status.is_server_error() { "internal error".to_string() } else { self.0.to_string() };
        let body = ApiError {
            error: ApiErrorBody {
                code: self.0.code().to_string(),
                message: msg,
                retryable: self.0.retryable(),
                details: serde_json::Value::Null,
            },
        };
        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
        }
        response
    }
}

pub(crate) fn ident(ext: Option<Extension<PeerIdentityExt>>) -> ApiResult<hh_net_tls::PeerIdentity> {
    ext.map(|Extension(p)| p.0).ok_or(Error::Unauthenticated.into())
}

pub(crate) fn require_scope(id: &hh_net_tls::PeerIdentity, scope: &str) -> ApiResult<()> {
    if id.has_scope(scope) {
        Ok(())
    } else {
        Err(Error::ForbiddenScope(scope.into()).into())
    }
}

fn require_transfer_owner(db: &hh_db::Db, transfer_id: &str, peer: &hh_net_tls::PeerIdentity) -> ApiResult<()> {
    let transfer = db.get_transfer(transfer_id)?.ok_or(Error::TransferGone)?;
    if transfer.device_id != peer.device_id {
        return Err(Error::TransferGone.into());
    }
    Ok(())
}

fn authorize_transfer_path(db: &hh_db::Db, path: &str, peer: &hh_net_tls::PeerIdentity) -> ApiResult<()> {
    if let Some(rest) = path.strip_prefix("/v1/transfers/") {
        require_transfer_owner(db, rest.split('/').next().unwrap_or(""), peer)?;
    }
    Ok(())
}

use crate::tls::{self as hh_net_tls, PeerIdentity};

#[derive(Clone)]
pub struct PeerIdentityExt(pub PeerIdentity);

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/info", get(get_info))
        .route("/v1/ping", get(ping))
        .route("/v1/status", get(get_status))
        .route("/v1/events", get(events))
        .route("/v1/devices", get(list_devices))
        .route("/v1/devices/{id}", delete(revoke_device))
        .route("/v1/devices/me", get(get_me).patch(update_me))
        .route("/v1/certs/renew", post(renew_cert))
        .route("/v1/transfers", post(create_transfer).get(list_transfers))
        .route("/v1/transfers/{id}", get(transfer_status).delete(abort_transfer))
        .route(
            "/v1/transfers/{id}/chunks/{n}",
            put(put_chunk).layer(axum::extract::DefaultBodyLimit::max(
                hh_core::MAX_CHUNK_SIZE as usize + 1024,
            )),
        )
        .route("/v1/transfers/{id}/complete", post(complete_transfer))
        .route("/v1/files", get(list_files))
        .route("/v1/files/{id}", get(get_file).patch(rename_file).delete(trash_file))
        .route("/v1/files/{id}/content", get(file_content))
        .route("/v1/files/{id}/manifest", get(file_manifest))
        .route("/v1/trash", get(list_trash))
        .route("/v1/trash/{id}/restore", post(restore_trash))
        .route("/v1/trash/{id}", delete(purge_trash))
        .route("/v1/library/summary", get(library_summary))
        .route("/v1/photos/timeline", get(photos_timeline))
        .route("/v1/photos/years", get(photos_years))
        .route("/v1/photos/{file_id}/thumb", get(photo_thumb))
        .route("/v1/backup/sources", post(backup_source_create).get(backup_source_list))
        .route("/v1/backup/sources/{id}", patch(backup_source_update))
        .route("/v1/backup/sources/{id}/diff", post(backup_diff))
        .route("/v1/backup/sources/{id}/summary", get(backup_summary))
        .route("/v1/backup/sources/{id}/verified-items", get(backup_verified_items))
        .route("/v1/backup/sources/{id}/cleanup-lease", post(cleanup_lease))
        .route("/v1/backup/sources/{id}/cleanup", get(cleanup_reviews))
        .route("/v1/backup/cleanup-leases/{id}/complete", post(cleanup_finish))
        .route("/v1/backup/items/{id}/confirm-local-freed", post(confirm_local_freed))
        .route("/v1/duplicates", get(list_duplicates))
        .route("/v1/duplicates/{group_id}/resolve", post(resolve_duplicates))
        .route("/v1/photos/similar", get(list_similar))
        .route("/v1/photos/similar/scan", post(scan_similar))
        .route("/v1/storage/health", get(storage_health))
        .route("/v1/alerts", get(list_alerts))
        .route("/v1/alerts/{id}/ack", post(ack_alert))
        .route("/v1/storage/second-copy/run", post(run_second_copy))
        .route("/v1/remote/input", get(remote_input_ws))
        .route("/v1/remote/power", post(remote_power))
        .route("/v1/remote/media", post(remote_media))
        .route("/v1/remote/wake-info", get(wake_info))
        .route("/v1/remote/wake", post(remote_wake))
        .route("/v1/network/hotspot", post(network_hotspot))
        .route("/v1/storage/encryption", get(storage_encryption))
        .route("/v1/security/audit", get(security_audit))
        .route("/v1/update/check", get(update_check))
        .route("/v1/screen/view", post(screen_start))
        .route("/v1/screen/cast", post(screen_start))
        .route("/v1/screen/{id}", delete(screen_stop).patch(screen_touch))
        .route("/v1/screen/capabilities", get(screen_capabilities))
        .route("/v1/hardware/audit", get(hardware_audit))
        .route("/v1/hardware/audit/run", post(hardware_audit_run))
        .route("/v1/network", get(network_info))
        .merge(crate::relay::router())
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state)
}

/// Revalidate certificates/scopes on every request, including keep-alive H2.
async fn authorize(State(s): State<AppState>, mut req: axum::extract::Request, next: axum::middleware::Next) -> ApiResult<Response> {
    let identity = req.extensions().get::<PeerIdentityExt>().map(|p|p.0.clone()).ok_or(Error::Unauthenticated)?;
    let device = s.db.device_by_id(&identity.device_id)?.ok_or(Error::Unauthenticated)?;
    if device.status != "active" || s.db.is_serial_revoked(&identity.cert_serial)? { return Err(Error::DeviceRevoked.into()); }
    if !s.db.device_accepts_serial(&identity.device_id,&identity.cert_serial)? { return Err(Error::Unauthenticated.into()); }
    if let Some(old)=s.db.activate_renewal(&identity.device_id,&identity.cert_serial)?{s.revocation.revoke(&old);}
    if s.db.get_setting("sharing.paused")?.as_deref() == Some("true") { return Err(Error::StorageUnavailable("sharing paused".into()).into()); }
    let path = req.uri().path();
    if !path.contains("/chunks/") && !s.rate_limiter.check(&identity.device_id) { return Err(Error::RateLimited.into()); }
    let fresh = PeerIdentity { scopes: device.scopes, ..identity };
    authorize_transfer_path(&s.db, path, &fresh)?;
    if let Some(rest) = path.strip_prefix("/v1/backup/sources/").or_else(||path.strip_prefix("/v1/backup/items/")) {
        let sid = rest.split('/').next().unwrap_or("");
        if s.photos.get_source(sid)?.device_id != fresh.device_id { return Err(Error::ForbiddenScope("backup owner".into()).into()); }
    }
    req.extensions_mut().insert(PeerIdentityExt(fresh));
    Ok(next.run(req).await)
}

#[derive(Deserialize)]
struct LeaseReq { client_item_ids: Vec<String>,client_review_id:Option<String> }
async fn cleanup_lease(State(s): State<AppState>, ext: Option<Extension<PeerIdentityExt>>, Path(id): Path<String>, Json(req): Json<LeaseReq>) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?; require_scope(&peer,"photos")?;
    Ok(Json(serde_json::json!(s.photos.create_cleanup_review(&id,&peer.device_id,&req.client_item_ids,req.client_review_id.as_deref())?)))
}
async fn cleanup_reviews(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>)->ApiResult<Json<serde_json::Value>> {let peer=ident(ext)?;require_scope(&peer,"photos")?;Ok(Json(serde_json::json!({"items":s.photos.active_cleanup_reviews(&id,&peer.device_id)?})))}
#[derive(Deserialize)]
struct LeaseFinishReq { freed_client_item_ids: Vec<String> }
async fn cleanup_finish(State(s): State<AppState>, ext: Option<Extension<PeerIdentityExt>>, Path(id): Path<String>, Json(req): Json<LeaseFinishReq>) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?; require_scope(&peer,"photos")?;
    s.photos.finish_cleanup_lease(&id,&peer.device_id,&req.freed_client_item_ids)?;
    Ok(Json(serde_json::json!({"completed":true})))
}

// ---- info / status ----

async fn get_info(State(s): State<AppState>) -> ApiResult<Json<HubInfo>> {
    let (hub_id, name) = s.db.hub_identity()?;
    let net = s.hw.network_info().unwrap_or(hh_hw::network::NetworkInfo {
        interfaces: vec![],
        mode: "unknown".into(),
    });
    Ok(Json(HubInfo {
        hub_id,
        name,
        version: HUB_VERSION.into(),
        api_min: API_VERSION,
        api_max: API_VERSION,
        features: FeaturesWire {
            photos: s.cfg.features.photos,
            remote: s.db.get_setting("remote.enabled")?.map(|v|v=="true").unwrap_or(s.cfg.features.remote) && s.helper.is_some(),
            screen: s.db.get_setting("screen.enabled")?.map(|v|v=="true").unwrap_or(s.cfg.features.screen) && s.helper.is_some(),
            hotspot: s.cfg.features.hotspot,
            wol: s.cfg.features.wol,
        },
        network: NetworkWire {
            kind: net.mode,
            link_mbps: None,
            wifi_standard: None,
            // W2.9: distinguish "no internet" from "no LAN". A LAN without
            // internet is a supported, healthy Home Hub state.
            internet: cached_internet_probe(),
        },
        time: hh_core::time::now_ms(),
        backup_interval_minutes: s.db.get_setting("backup.interval_minutes")?.and_then(|v|v.parse::<u64>().ok()).unwrap_or(60).clamp(1,10080),
    }))
}

/// 60 s cache so /v1/info stays cheap (probe itself is a 2 s connect).
fn cached_internet_probe() -> Option<bool> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    static CACHE: OnceLock<Mutex<(Option<Instant>, Option<bool>)>> = OnceLock::new();
    let cell = CACHE.get_or_init(|| Mutex::new((None, None)));
    let mut g = cell.lock().ok()?;
    let fresh = g.0.map(|t| t.elapsed() < std::time::Duration::from_secs(60)).unwrap_or(false);
    if !fresh {
        g.1 = hh_hw::network::probe_internet();
        g.0 = Some(Instant::now());
    }
    g.1
}

#[derive(Deserialize)]
struct PingQ {
    bytes: Option<usize>,
}

async fn ping(Query(q): Query<PingQ>) -> Response {
    let n = q.bytes.unwrap_or(0).min(8 * 1024 * 1024);
    Response::builder()
        .header("content-type", "application/octet-stream")
        .body(Body::from(vec![0u8; n]))
        .unwrap()
}

async fn get_status(State(s): State<AppState>) -> ApiResult<Json<HubStatus>> {
    let storage=s.storage.clone();
    let sum=tokio::task::spawn_blocking(move||storage.library_summary()).await.map_err(|e|Error::Internal(e.to_string()))??;
    let active = s.db.list_transfers(None, Some("open"))?.len() as u32;
    let health = s
        .storage
        .health_summary()?
        .first()
        .map(|d| d.health.clone())
        .unwrap_or_else(|| "unknown".into());
    Ok(Json(HubStatus {
        online: true,
        free_bytes: sum.free_bytes,
        total_bytes: sum.total_bytes,
        active_transfers: active,
        alerts_count: s.db.active_alert_count()?,
        health_summary: health,
    }))
}

async fn events(State(s): State<AppState>, ext: Option<Extension<PeerIdentityExt>>) -> ApiResult<Sse<impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>>> {
    let peer = ident(ext)?;
    let rx = s.events.subscribe();
    let stream = futures::stream::unfold((rx,s.db.clone(),peer), |(mut rx,db,peer)| async move {
        loop {
            let device = db.device_by_id(&peer.device_id).ok().flatten()?;
            if device.status != "active" || device.cert_serial != peer.cert_serial || device.cert_expires_at <= hh_core::time::now_ms() || db.get_setting("sharing.paused").ok().flatten().as_deref() == Some("true") { return None; }
            match tokio::time::timeout(std::time::Duration::from_secs(1),rx.recv()).await {
                Ok(Ok(ev)) => {
                    if let Some(tid) = ev.data.get("transfer_id").and_then(|v|v.as_str()) {
                        if db.get_transfer(tid).ok().flatten().map(|t|t.device_id != peer.device_id).unwrap_or(true) { continue; }
                    }
                    let sse = SseEvent::default().event(ev.event).data(ev.data.to_string());
                    return Some((Ok(sse),(rx,db,peer)));
                },
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(_)) => return None,
                Err(_) => continue,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

// ---- devices ----

async fn list_devices(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Vec<Device>>> {
    let id = ident(ext)?;
    require_scope(&id, "admin")?;
    Ok(Json(s.db.list_devices()?.iter().map(|d| d.to_api()).collect()))
}

async fn get_me(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>)->ApiResult<Json<serde_json::Value>> {
    let peer=ident(ext)?;let d=s.db.device_by_id(&peer.device_id)?.ok_or(Error::Unauthenticated)?;
    Ok(Json(serde_json::json!({"id":d.id,"name":d.name,"scopes":d.scopes})))
}

async fn revoke_device(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(device_id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = ident(ext)?;
    require_scope(&id, "admin")?;
    let dev = s
        .db
        .device_by_id(&device_id)?
        .ok_or_else(|| Error::NotFound(device_id.clone()))?;
    s.db.revoke_device(&device_id, "user")?;
    s.revocation.revoke(&dev.cert_serial);
    let stopping = s.clone();
    let revoked_id = device_id.clone();
    tokio::task::spawn_blocking(move || stopping.stop_device_screens(&revoked_id))
        .await.map_err(|e| Error::Internal(e.to_string()))?;
    s.db.audit(Some(&id.device_id), "revoke", Some(&device_id), None)?;
    s.events.emit("device.revoked", serde_json::json!({"device_id": device_id}));
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct UpdateMe {
    name: Option<String>,
    app_version: Option<String>,
}

async fn update_me(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(body): Json<UpdateMe>,
) -> ApiResult<StatusCode> {
    let id = ident(ext)?;
    let c = s.db.lock()?;
    if let Some(name) = &body.name {
        let name = hh_core::paths::sanitize_component(name)?;
        c.execute(
            "UPDATE devices SET name=?2 WHERE id=?1",
            rusqlite::params![id.device_id, name],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
    }
    if let Some(v) = &body.app_version {
        c.execute(
            "UPDATE devices SET app_version=?2 WHERE id=?1",
            rusqlite::params![id.device_id, v],
        )
        .map_err(|e| Error::Db(e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn renew_cert(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    body: String,
) -> ApiResult<Json<serde_json::Value>> {
    let id = ident(ext)?;
    let dev = s
        .db
        .device_by_id(&id.device_id)?
        .ok_or(Error::Unauthenticated)?;
    let ms_left = dev.cert_expires_at - hh_core::time::now_ms();
    if ms_left > 30 * 24 * 3600 * 1000 {
        return Err(Error::Conflict("cert not due for renewal (>30 days left)".into()).into());
    }
    let csr = if body.trim_start().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(&body).map_err(|_|Error::BadRequest("invalid renewal request".into()))?
            .get("csr_pem").and_then(|v|v.as_str()).ok_or_else(||Error::BadRequest("csr_pem required".into()))?.to_owned()
    } else { body };
    if let Some((cert_pem,expires))=s.db.pending_renewal(&id.device_id,&id.cert_serial,&csr)? {return Ok(Json(serde_json::json!({"cert_pem":cert_pem,"cert_expires_at":expires})));}
    let (cert_pem, serial, expires) = s.ca.renew_device_cert(&csr, &id.device_id)?;
    s.db.stage_renewal(&id.device_id,&id.cert_serial,&csr,&cert_pem,&serial,expires)?;
    Ok(Json(serde_json::json!({"cert_pem": cert_pem, "cert_expires_at": expires})))
}

// ---- transfers ----

async fn create_transfer(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<CreateTransferRequest>,
) -> ApiResult<(StatusCode, Json<CreateTransferResponse>)> {
    let id = ident(ext)?;
    require_scope(&id, "transfer")?;
    if req.size > 1 << 40 {
        return Err(Error::TooLarge("max file size 1 TiB".into()).into());
    }
    if req.backup_source_id.is_some() { require_scope(&id,"photos")?; }
    if let Some(target)=&req.target_device_id {
        let d=s.db.device_by_id(target)?.ok_or_else(||Error::NotFound("recipient".into()))?;
        if d.status!="active" || !d.has_scope("files") {return Err(Error::ForbiddenScope("recipient files".into()).into());}
    }
    let engine = s.transfers.clone();
    let device_id = id.device_id.clone();
    let input = req.clone();
    let mut resp = tokio::task::spawn_blocking(move ||engine.create(&device_id,&input)).await.map_err(|e|Error::Internal(e.to_string()))??;
    if let (Some(source),Some(item),Some(file_id),Some(hash)) = (&req.backup_source_id,&req.client_item_id,&resp.existing_file_id,&req.root_hash) {
        resp.backup_item_id = Some(s.photos.mark_transfer_verified(source,&id.device_id,item,file_id,hash,None)?);
    }
    if let (Some(target),Some(file))=(&req.target_device_id,&resp.existing_file_id) {crate::relay::stage_file(&s,file,&id.device_id,target,None)?;}
    let code = if resp.have.count() > 0 { StatusCode::OK } else { StatusCode::CREATED };
    Ok((code, Json(resp)))
}

async fn transfer_status(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<Json<TransferStatus>> {
    let peer = ident(ext)?;
    require_scope(&peer, "transfer")?;
    Ok(Json(s.transfers.status(&id)?))
}

async fn list_transfers(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Vec<TransferStatus>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "transfer")?;
    let rows = s.db.list_transfers(Some(&peer.device_id), None)?;
    let mut out = Vec::new();
    for t in rows {
        let have = s.db.chunk_bitmap(&t.id)?;
        out.push(t.to_status(have));
    }
    Ok(Json(out))
}

async fn put_chunk(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path((id, n)): Path<(String, u64)>,
    headers: HeaderMap,
    body: bytes::Bytes,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "transfer")?;
    let hash = headers
        .get("x-chunk-hash")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Error::BadRequest("missing X-Chunk-Hash".into()))?;
    let engine = s.transfers.clone();
    let body = body.to_vec();
    let hash = hash.to_string();
    let id2 = id.clone();
    // Hashing is CPU-heavy: keep it off the reactor (AGENTS.md §4).
    tokio::task::spawn_blocking(move || engine.put_chunk(&id2, n, &hash, &body))
        .await
        .map_err(|e| Error::Internal(e.to_string()))??;
    let st = s.transfers.status(&id)?;
    s.events.emit(
        "transfer.progress",
        serde_json::json!({"transfer_id": id, "bytes_verified": st.bytes_verified, "size": st.size}),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn complete_transfer(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    Json(req): Json<CompleteRequest>,
) -> ApiResult<Json<CompleteResponse>> {
    let peer = ident(ext)?;
    require_scope(&peer, "transfer")?;
    let owner:String={let c=s.db.lock()?;c.query_row("SELECT device_id FROM transfers WHERE id=?1",[&id],|r|r.get(0)).map_err(|_|Error::TransferGone)?};
    if owner!=peer.device_id {return Err(Error::ForbiddenScope("transfer".into()).into());}
    let engine = s.transfers.clone();
    let id2 = id.clone();
    let mut resp = tokio::task::spawn_blocking(move || engine.complete(&id2, &req.root_hash))
        .await
        .map_err(|e| Error::Internal(e.to_string()))??;
    let backup: (Option<String>,Option<String>) = {
        let c = s.db.lock()?;
        c.query_row("SELECT backup_source_id,client_item_id FROM transfers WHERE id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|Error::Db(e.to_string()))?
    };
    if let (Some(source),Some(item)) = backup {
        resp.backup_item_id = Some(s.photos.mark_transfer_verified(&source,&peer.device_id,&item,&resp.file_id,&resp.hash,Some(&id))?);
    }
    let target:Option<String>={let c=s.db.lock()?;c.query_row("SELECT target_device_id FROM transfers WHERE id=?1",[&id],|r|r.get(0)).map_err(|e|Error::Db(e.to_string()))?};
    if let Some(target)=target {crate::relay::stage_file(&s,&resp.file_id,&peer.device_id,&target,Some(&id))?;}
    // Media hook: register metadata + enqueue thumbnails (TRD §8).
    if resp.rel_path.starts_with("Photos/") || resp.rel_path.starts_with("Videos/") {
        let path = s.storage.file_disk_path(&resp.file_id)?;
        let mime = if resp.rel_path.starts_with("Photos/") { Some("image/*") } else { Some("video/*") };
        if let Ok(meta) = hh_photos::meta::extract(&path, mime) {
            let type_ = if resp.rel_path.starts_with("Photos/") { "photo" } else { "video" };
            let _ = s.photos.register_media(&resp.file_id, type_, &meta);
        }
    }
    s.events.emit(
        "transfer.completed",
        serde_json::json!({"transfer_id": id, "file_id": resp.file_id, "verified": true}),
    );
    Ok(Json(resp))
}

async fn abort_transfer(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "transfer")?;
    s.transfers.abort(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- files ----

#[derive(Deserialize)]
struct FilesQ {
    category: Option<String>,
    q: Option<String>,
    cursor: Option<String>,
    sort: Option<String>,
    limit: Option<u32>,
}

async fn list_files(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Query(q): Query<FilesQ>,
) -> ApiResult<Json<Page<FileObject>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    Ok(Json(s.storage.list_files_page(q.category.as_deref(), q.q.as_deref(), q.cursor.as_deref(), q.sort.as_deref(), q.limit.unwrap_or(100))?))
}

async fn get_file(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<Json<FileObject>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    Ok(Json(s.storage.get_file(&id)?))
}

async fn file_content(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    let meta = s.storage.get_file(&id)?;
    let path = s.storage.file_disk_path(&id)?;

    stream_file(path, &meta, &headers).await
}

/// Streaming bounded-memory downloads, shared by local and mTLS routes.
pub(crate) async fn stream_file(path: std::path::PathBuf, meta: &FileObject, headers: &HeaderMap) -> ApiResult<Response> {
    use tokio::io::{AsyncReadExt,AsyncSeekExt};
    let mut file = tokio::fs::File::open(path).await.map_err(Error::from)?;
    let range = headers.get("range").map(|r|r.to_str().ok().and_then(|v|parse_range(v,meta.size)));
    if matches!(range,Some(None)) {
        return Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE).header("content-range",format!("bytes */{}",meta.size))
            .body(Body::empty()).map_err(|e|Error::Internal(e.to_string()).into());
    }
    let (start,len,status) = match range.flatten() { Some((a,b)) => (a,b-a+1,StatusCode::PARTIAL_CONTENT),None => (0,meta.size,StatusCode::OK) };
    file.seek(std::io::SeekFrom::Start(start)).await.map_err(Error::from)?;
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(file.take(len)));
    let filename = meta.name.replace(['"','\\','\r','\n'],"_");
    let mut response = Response::builder().status(status).header("content-type",meta.mime.as_deref().unwrap_or("application/octet-stream"))
        .header("content-length",len).header("accept-ranges","bytes").header("x-content-type-options","nosniff")
        .header("content-security-policy","sandbox").header("content-disposition",format!("attachment; filename=\"{filename}\""));
    if status == StatusCode::PARTIAL_CONTENT { response = response.header("content-range",format!("bytes {start}-{}/{}",start+len-1,meta.size)); }
    response.body(body).map_err(|e|Error::Internal(e.to_string()).into())
}

fn parse_range(header: &str, size: u64) -> Option<(u64,u64)> {
    if size == 0 { return None; }
    let spec = header.strip_prefix("bytes=")?;
    if spec.contains(',') { return None; }
    let (a,b) = spec.split_once('-')?;
    if a.is_empty() {
        let suffix: u64 = b.parse().ok()?;
        if suffix == 0 { return None; }
        return Some((size.saturating_sub(suffix),size-1));
    }
    let start: u64 = a.parse().ok()?;
    let end: u64 = if b.is_empty() {size-1} else { b.parse::<u64>().ok()?.min(size-1) };
    if start >= size || start > end {None} else {Some((start,end))}
}

async fn file_manifest(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    let chunks = s.storage.file_manifest(&id)?;
    Ok(Json(serde_json::json!({"file_id": id, "chunks": chunks.iter().map(|(i, h)| serde_json::json!({"idx": i, "hash": h})).collect::<Vec<_>>()})))
}

#[derive(Deserialize)]
struct RenameReq {
    name: String,
}

async fn rename_file(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    Json(req): Json<RenameReq>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    s.storage.rename_file(&id, &req.name)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn trash_file(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    s.storage.trash_file(&id, Some(&peer.device_id))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_trash(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Page<FileObject>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    Ok(Json(s.storage.list_trash(500)?))
}

async fn restore_trash(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    s.storage.restore_file(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_trash(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    s.storage.purge_file(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn library_summary(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<hh_storage::LibrarySummary>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    let storage=s.storage.clone();
    Ok(Json(tokio::task::spawn_blocking(move||storage.library_summary()).await.map_err(|e|Error::Internal(e.to_string()))??))
}

// ---- photos & backup ----

#[derive(Deserialize)]
struct TimelineQ {
    cursor: Option<String>,
    limit: Option<u32>,
    year: Option<i32>,
    month: Option<u32>,
}

async fn photos_timeline(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Query(q): Query<TimelineQ>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    let cursor = q.cursor.and_then(|c| {
        let (ts, fid) = c.split_once(':')?;
        Some((ts.parse().ok()?, fid.to_string()))
    });
    let month=match (q.year,q.month) {
        (None,None)=>None,
        (Some(y),Some(m)) if (1900..=2200).contains(&y) && (1..=12).contains(&m)=>Some((y,m)),
        _=>return Err(Error::BadRequest("invalid gallery month".into()).into()),
    };
    let (items, next_cursor) = s.photos.timeline_month(cursor, q.limit.unwrap_or(100),month)?;
    Ok(Json(serde_json::json!({"items": items, "next_cursor": next_cursor})))
}

async fn photos_years(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Vec<hh_photos::gallery::YearMonthCount>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    Ok(Json(s.photos.years()?))
}

#[derive(Deserialize)]
struct ThumbQ {
    size: Option<u32>,
}

async fn photo_thumb(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(file_id): Path<String>,
    Query(q): Query<ThumbQ>,
) -> ApiResult<Response> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    match s.photos.thumb_path(&file_id, q.size.unwrap_or(256))? {
        Some(p) => {
            let bytes = match tokio::fs::read(&p).await {
                Ok(bytes)=>bytes,
                Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Response::builder().status(StatusCode::NO_CONTENT).body(Body::empty()).unwrap()),
                Err(e)=>return Err(Error::from(e).into()),
            };
            Ok(Response::builder()
                .header("content-type", "image/jpeg")
                .body(Body::from(bytes))
                .unwrap())
        }
        None => Ok(Response::builder().status(StatusCode::NO_CONTENT).body(Body::empty()).unwrap()),
    }
}

#[derive(Deserialize)]
struct SourceReq {
    kind: String,
    label: Option<String>,
}

async fn backup_source_create(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<SourceReq>,
) -> ApiResult<Json<hh_photos::backup::BackupSource>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    Ok(Json(s.photos.upsert_source(&peer.device_id, &req.kind, req.label.as_deref())?))
}

#[derive(Deserialize)]
struct SourcePatch {
    enabled: Option<bool>,
    wifi_only: Option<bool>,
    charging_only: Option<bool>,
}

async fn backup_source_update(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    Json(req): Json<SourcePatch>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    s.photos.update_source(&id, req.enabled, req.wifi_only, req.charging_only)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn backup_diff(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    Json(items): Json<Vec<hh_photos::backup::DiffItem>>,
) -> ApiResult<Json<hh_photos::backup::DiffResponse>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    Ok(Json(s.photos.diff(&id, &items)?))
}

async fn backup_summary(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<Json<hh_photos::backup::BackupSummary>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    let photos=s.photos.clone();
    Ok(Json(tokio::task::spawn_blocking(move ||photos.summary(&id))
        .await.map_err(|e|Error::Internal(e.to_string()))??))
}

/// The caller's own backup sources (free-storage flow, W1.3).
async fn backup_source_list(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    let rows = s.db.backup_sources_for_device(&peer.device_id)?;
    Ok(Json(serde_json::json!({
        "sources": rows.into_iter().map(|(id, label)| serde_json::json!({"id": id, "label": label})).collect::<Vec<_>>()
    })))
}

/// Verified-but-not-yet-freed items for one source (FR-5.3). The client
/// re-verifies locally (existence + size) before deleting from MediaStore.
async fn backup_verified_items(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    // Only the owning device may enumerate its verified items.
    let owned = s.db.backup_sources_for_device(&peer.device_id)?.iter().any(|(sid, _)| *sid == id);
    if !owned {
        return Err(Error::ForbiddenScope("photos".into()).into());
    }
    let sum = s.photos.summary(&id)?;
    let items = s.db.backup_verified_items(&id, 500)?;
    Ok(Json(serde_json::json!({
        "source_id": id,
        "summary": sum,
        "items": items.iter().map(|i| serde_json::json!({
            "client_item_id": i.client_item_id,
            "file_id": i.file_id,
            "size": i.size,
            "verified_at": i.verified_at,
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct FreedReq {
    client_item_id: String,
}

async fn confirm_local_freed(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
    Json(req): Json<FreedReq>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    // `id` here is the source id; the client_item_id identifies the item.
    s.photos.confirm_local_freed(&id, &req.client_item_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- duplicates ----

async fn list_duplicates(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Vec<hh_storage::dedupe::DuplicateGroup>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    Ok(Json(s.storage.list_duplicates()?))
}

#[derive(Deserialize)]
struct ResolveReq {
    keep_file_id: String,
}

async fn resolve_duplicates(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(group_id): Path<String>,
    Json(req): Json<ResolveReq>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "photos")?;
    s.storage.resolve_duplicate_group(&group_id, &req.keep_file_id)?;
    let _ = s.db.audit(Some(&peer.device_id), "resolve_duplicates", Some(&group_id), None);
    Ok(StatusCode::NO_CONTENT)
}

// ---- similar (perceptual) duplicates, W2.3 ----

async fn list_similar(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<Vec<hh_storage::perceptual::SimilarGroup>>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    Ok(Json(s.storage.list_similar()?))
}

async fn scan_similar(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    let st = s.storage.clone();
    let created = tokio::task::spawn_blocking(move || st.scan_similar())
        .await
        .map_err(|e| Error::Internal(e.to_string()))??;
    Ok(Json(serde_json::json!({"groups_created": created})))
}

// ---- storage health & alerts ----

async fn storage_health(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "files")?;
    Ok(Json(serde_json::json!({
        "disks": s.storage.health_summary()?,
        "second_copy_age_ms": s.storage.last_second_copy_age_ms()?,
    })))
}

async fn list_alerts(
    State(s): State<AppState>,
    _ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let c = s.db.lock()?;
    let mut st = c
        .prepare("SELECT id, severity, code, message, created_at FROM alerts WHERE resolved_at IS NULL ORDER BY created_at DESC")
        .map_err(|e| Error::Db(e.to_string()))?;
    let items: Vec<serde_json::Value> = st
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "severity": r.get::<_, String>(1)?,
                "code": r.get::<_, String>(2)?,
                "message": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| Error::Db(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Db(e.to_string()))?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn ack_alert(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let _ = ident(ext)?;
    let c = s.db.lock()?;
    c.execute(
        "UPDATE alerts SET acknowledged_at=?2 WHERE id=?1",
        rusqlite::params![id, hh_core::time::now_ms()],
    )
    .map_err(|e| Error::Db(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_second_copy(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    let svc = s.storage.clone();
    let (copied, failed, run) =
        tokio::task::spawn_blocking(move || svc.run_second_copy())
            .await
            .map_err(|e| Error::Internal(e.to_string()))??;
    Ok(Json(serde_json::json!({"copied": copied, "failed": failed, "run_id": run})))
}

// ---- remote ----

async fn remote_input_ws(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    let peer = ident(ext)?;
    require_scope(&peer, "remote")?;
    let session_id = s.remote.record_session(&peer.device_id, "input")?;
    Ok(ws.max_message_size(8192).max_frame_size(8192).on_upgrade(move |mut socket| async move {
        let remote = s.remote.clone();
        let device = peer.device_id.clone();
        let idle = std::time::Duration::from_millis(hh_remote::IDLE_TIMEOUT_MS as u64);
        let mut deadline=tokio::time::Instant::now()+idle;
        loop {
            let message=tokio::select! {
                message=tokio::time::timeout_at(deadline,socket.recv())=>message,
                _=tokio::time::sleep(std::time::Duration::from_secs(1))=>{
                    let allowed=s.db.device_by_id(&device).ok().flatten().is_some_and(|d|d.status=="active"&&d.cert_serial==peer.cert_serial&&d.has_scope("remote"))&&s.db.get_setting("sharing.paused").ok().flatten().as_deref()!=Some("true");
                    if !allowed{let _=socket.close().await;break;}continue;
                }
            };
            match message {
                Ok(Some(Ok(axum::extract::ws::Message::Text(text)))) => {
                    deadline=tokio::time::Instant::now()+idle;
                    let allowed = s.db.device_by_id(&device).ok().flatten().map(|d|d.status == "active" && d.cert_serial == peer.cert_serial && d.has_scope("remote")).unwrap_or(false)
                        && s.db.get_setting("sharing.paused").ok().flatten().as_deref() != Some("true");
                    if !allowed { let _ = socket.close().await; break; }
                    if let Err(e) = remote.handle_input(&device, text.as_str()) {
                        let _ = socket
                            .send(axum::extract::ws::Message::Text(
                                serde_json::json!({"error": e.code()}).to_string().into(),
                            ))
                            .await;
                    }
                }
                Ok(Some(Ok(axum::extract::ws::Message::Close(_)))) | Ok(None) => break,
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(_))) => break,
                Err(_) => break, // idle 60 s → auto-end (API_SPEC §8)
            }
        }
        if let Some(h)=s.helper.clone(){let _=tokio::task::spawn_blocking(move||h.release_input()).await;}
        let _ = remote.end_session(&session_id);
    }))
}

#[derive(Deserialize)]
struct PowerReq {
    action: String,
    confirm: bool,
}

async fn remote_power(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<PowerReq>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "remote")?;
    s.remote.power_action(&peer.device_id, &req.action, req.confirm)?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
struct MediaReq {
    key: String,
}

async fn remote_media(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<MediaReq>,
) -> ApiResult<StatusCode> {
    let peer = ident(ext)?;
    require_scope(&peer, "remote")?;
    s.remote.media_key(&peer.device_id, &req.key)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn wake_info(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let _ = ident(ext)?;
    let macs = s.hw.wake_info()?;
    Ok(Json(serde_json::json!({
        "macs": macs.iter().map(|(i, m)| serde_json::json!({"iface": i, "mac": m})).collect::<Vec<_>>(),
        "capability": if s.cfg.features.wol { "supported_where_nic_allows" } else { "unknown" },
        "note": "Wake is best-effort; queued transfers are the guarantee."
    })))
}

/// Wake-from-phone (W2.2, FR-8.x): send WoL magic packets to the hub's
/// recorded NICs. Best-effort by design (D3); every attempt is audited.
async fn remote_wake(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "remote")?;
    if !s.cfg.features.wol {
        return Err(Error::ForbiddenScope("wol".into()).into());
    }
    let macs = s.hw.wake_info()?;
    let mut sent: Vec<String> = vec![];
    let mut failed: Vec<String> = vec![];
    for (_iface, mac) in &macs {
        match hh_hw::wake::send_wake(mac, "255.255.255.255") {
            Ok(()) => sent.push(mac.clone()),
            Err(e) => {
                tracing::warn!(mac = %mac, error = %e, "wake packet failed");
                failed.push(mac.clone());
            }
        }
    }
    let _ = s.db.audit(Some(&peer.device_id), "remote_wake", Some(&format!("sent:{} failed:{}", sent.len(), failed.len())), None);
    Ok(Json(serde_json::json!({"sent": sent, "failed": failed})))
}

/// Mobile-hotspot toggle (W2.7, FR-8.3): forwarded to the session helper's
/// WinRT tethering manager. Requires features.hotspot AND a running helper.
#[derive(Deserialize)]
struct HotspotReq {
    enable: bool,
    #[serde(default)]
    ssid: Option<String>,
    #[serde(default)]
    passphrase: Option<String>,
}

async fn network_hotspot(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<HotspotReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    if !s.cfg.features.hotspot {
        return Err(Error::ForbiddenScope("hotspot".into()).into());
    }
    let helper = s.helper.clone().ok_or_else(|| {
        Error::Internal("session helper not running; hotspot requires hh-session".into())
    })?;
    let enable = req.enable;
    let result = tokio::task::spawn_blocking(move || {
        helper.hotspot(enable, req.ssid.as_deref(), req.passphrase.as_deref())
    })
    .await
    .map_err(|e| Error::Internal(e.to_string()))??;
    let _ = s.db.audit(Some(&peer.device_id), "hotspot_toggle", Some(if enable { "on" } else { "off" }), None);
    Ok(Json(result))
}

/// BitLocker detect/recommend (W2.8, FR-4.5). Honest null on non-Windows or
/// when the helper is absent — the dashboard shows a recommendation banner.
async fn storage_encryption(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let _ = ident(ext)?;
    let status = match &s.helper {
        Some(h) => {
            let h = h.clone();
            tokio::task::spawn_blocking(move || h.bitlocker_status()).await.ok()
        }
        None => None,
    };
    let value = match status {
        Some(Ok(v)) => v,
        Some(Err(e)) => serde_json::json!({ "available": false, "reason": e.to_string() }),
        None => serde_json::json!({ "available": false, "reason": "session helper not running" }),
    };
    Ok(Json(value))
}

/// Security audit log (FR-2.6, W1.4): admin scope, newest-first page.
#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    device_id: Option<String>,
}

async fn security_audit(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    let (rows, total) = s.db.list_audit(q.device_id.as_deref(), q.limit.unwrap_or(100), q.offset.unwrap_or(0))?;
    Ok(Json(serde_json::json!({
        "total": total,
        "items": rows.iter().map(|r| serde_json::json!({
            "id": r.id, "ts": r.ts, "device_id": r.device_id,
            "action": r.action, "detail": r.detail, "ip": r.ip,
        })).collect::<Vec<_>>()
    })))
}

/// Update channel (W2.6): fetch + verify the signed manifest.
async fn update_check(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<crate::update::UpdateCheck>> {
    let _ = ident(ext)?;
    let db = s.db.clone();
    let check = tokio::task::spawn_blocking(move || crate::update::check_from_settings(&db, HUB_VERSION))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    Ok(Json(check))
}

// ---- screen sharing (Phase 3) ----

#[derive(Deserialize)]
struct ScreenStartReq {
    offer_sdp: String,
    direction: Option<String>,
    preset: Option<hh_stream::Preset>,
}

async fn screen_start(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
    Json(req): Json<ScreenStartReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "remote")?;
    let rating=s.hw.latest_audit()?.and_then(|v|v.get("rating_streaming").and_then(|r|r.as_str().map(String::from))).unwrap_or_else(||"limited".into());
    let kind=req.direction.as_deref().unwrap_or("view");
    if !["view","cast"].contains(&kind){return Err(Error::BadRequest("direction must be view or cast".into()).into());}
    if !s.db.get_setting("screen.enabled")?.map(|v|v=="true").unwrap_or(s.cfg.features.screen) {return Err(Error::ForbiddenScope("screen disabled".into()).into());}
    let svc=s.stream.clone();let device=peer.device_id.clone();let kind=kind.to_string();let preset=req.preset;
    let (id,preset,answer)=tokio::task::spawn_blocking(move||svc.start_session(&device,&kind,preset,&rating,&req.offer_sdp)).await.map_err(|e|Error::Internal(e.to_string()))??;
    s.screen_owners.lock().map_err(|_|Error::Internal("screen mutex".into()))?.insert(id.clone(),peer.device_id);
    Ok(Json(serde_json::json!({"session_id":id,"preset":preset,"answer_sdp":answer,"transport":"helper"})))
}
async fn screen_touch(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>)->ApiResult<StatusCode>{let peer=ident(ext)?;require_scope(&peer,"remote")?;s.stream.touch_session(&id,&peer.device_id)?;Ok(StatusCode::NO_CONTENT)}
async fn screen_stop(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>)->ApiResult<StatusCode>{
    let peer=ident(ext)?;require_scope(&peer,"remote")?;
    let owner=s.screen_owners.lock().map_err(|_|Error::Internal("screen mutex".into()))?.get(&id).cloned();
    if owner.as_deref()!=Some(&peer.device_id){return Err(Error::ForbiddenScope("screen owner".into()).into());}
    let stream=s.stream.clone();let sid=id.clone();tokio::task::spawn_blocking(move||stream.stop_session(&sid)).await.map_err(|e|Error::Internal(e.to_string()))??;
    s.screen_owners.lock().map_err(|_|Error::Internal("screen mutex".into()))?.remove(&id);Ok(StatusCode::NO_CONTENT)
}

async fn screen_capabilities(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<hh_stream::ScreenCapabilities>> {
    let peer=ident(ext)?;require_scope(&peer,"remote")?;
    let helper=s.helper.clone();
    let caps=tokio::task::spawn_blocking(move||helper.ok_or_else(||Error::BadRequest("screen helper unavailable".into()))?.screen_capabilities()).await.map_err(|e|Error::Internal(e.to_string()))??;
    let encoders=caps["encoders"].as_array().map(|a|a.iter().filter_map(|v|v.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    let max_preset=serde_json::from_value(caps["max_preset"].clone()).unwrap_or(hh_stream::Preset::Low);
    Ok(Json(hh_stream::ScreenCapabilities{encoders,max_preset,hw_encode:caps["hw_encode"].as_bool().unwrap_or(false)}))
}

// ---- hardware & network ----

async fn hardware_audit(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let _ = ident(ext)?;
    let audit = s.hw.latest_audit()?.unwrap_or(serde_json::Value::Null);
    Ok(Json(audit))
}

async fn hardware_audit_run(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<serde_json::Value>> {
    let peer = ident(ext)?;
    require_scope(&peer, "admin")?;
    let hw = s.hw.clone();
    let helper=s.helper.clone();
    let r = tokio::task::spawn_blocking(move || {let extra=helper.and_then(|h|h.system_info().ok());hw.run_audit_with_helper(extra.as_ref())})
        .await
        .map_err(|e| Error::Internal(e.to_string()))??;
    Ok(Json(serde_json::json!({
        "id": r.id,
        "ratings": r.ratings,
        "supported_status": r.supported_status,
    })))
}

async fn network_info(
    State(s): State<AppState>,
    ext: Option<Extension<PeerIdentityExt>>,
) -> ApiResult<Json<hh_hw::network::NetworkInfo>> {
    let _ = ident(ext)?;
    Ok(Json(s.hw.network_info()?))
}

#[cfg(test)]
mod api_contract_tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn rate_limited_response_includes_retry_after() {
        let response = ApiErr(Error::RateLimited).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(axum::http::header::RETRY_AFTER).unwrap(), "1");
    }

    #[test]
    fn storage_full_error_is_507() {
        assert_eq!(Error::StorageFull.http_status(), 507);
        assert_eq!(Error::StorageFull.code(), "STORAGE_FULL");
    }
}

#[cfg(test)]
mod transfer_authorization_tests {
    use super::*;
    use hh_db::DeviceRow;

    #[test]
    fn another_device_cannot_address_a_transfer() {
        let db = hh_db::Db::open_memory().unwrap();
        for device_id in ["owner", "other"] {
            db.insert_device(&DeviceRow {
                id: device_id.into(), name: device_id.into(), platform: "android".into(),
                model: None, app_version: None, cert_serial: device_id.into(),
                cert_expires_at: 0, scopes: vec!["transfer".into()], paired_at: 0,
                last_seen_at: None, status: "active".into(),
            }, "PEM", "PUB").unwrap();
        }
        db.insert_transfer("transfer-1", "owner", "send", "photo.jpg", 1,
            None, None, 1, 1, None, None, "unused.part", i64::MAX).unwrap();
        let identity = |device_id: &str| PeerIdentity {
            device_id: device_id.into(), cert_serial: device_id.into(),
            scopes: vec!["transfer".into()],
        };
        for path in ["/v1/transfers/transfer-1", "/v1/transfers/transfer-1/chunks/0",
            "/v1/transfers/transfer-1/complete"] {
            assert!(authorize_transfer_path(&db, path, &identity("owner")).is_ok());
            let denial = authorize_transfer_path(&db, path, &identity("other")).unwrap_err();
            assert_eq!(denial.0.http_status(), 410);
            assert!(matches!(denial.0, Error::TransferGone));
        }
        assert!(matches!(authorize_transfer_path(&db, "/v1/transfers/missing", &identity("owner")),
            Err(ApiErr(Error::TransferGone))));
        assert!(authorize_transfer_path(&db, "/v1/transfers", &identity("other")).is_ok());
    }
}
