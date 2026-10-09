//! Authorized same-machine administration; composed under dashboard guards.
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use hh_core::{Error, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::dashboard::DashboardState;
use crate::routes::ApiErr;

type AdminResult<T> = std::result::Result<T, ApiErr>;

pub fn router() -> Router<DashboardState> {
    Router::new()
        .route("/api/session",post(session))
        .route("/api/setup",get(settings).post(save_settings))
        .route("/api/settings",get(settings).patch(save_settings))
        .route("/api/pause-sharing",post(pause))
        .route("/api/pair/pending",get(pair_pending))
        .route("/api/pair/confirm",post(pair_confirm))
        .route("/api/devices/{id}",axum::routing::patch(device_patch).delete(device_revoke))
        .route("/api/capabilities",get(capabilities))
        .route("/api/hardware/audit",get(hardware))
        .route("/api/hardware/audit/run",post(hardware_run))
        .route("/api/transfers/{id}",delete(transfer_abort))
        .route("/api/files/{id}",axum::routing::patch(file_rename).delete(file_trash))
        .route("/api/files/{id}/move",post(file_move))
        .route("/api/files/{id}/copy",post(file_copy))
        .route("/api/trash",get(trash))
        .route("/api/trash/{id}/restore",post(restore))
        .route("/api/trash/{id}",delete(purge))
        .route("/api/photos/timeline",get(timeline))
        .route("/api/photos/years",get(years))
        .route("/api/duplicates",get(duplicates))
        .route("/api/duplicates/scan",post(duplicate_scan))
        .route("/api/duplicates/{id}/resolve",post(duplicate_resolve))
        .route("/api/import/scan",post(import_scan))
        .route("/api/import",post(import_start))
        .route("/api/jobs",get(jobs))
        .route("/api/jobs/{id}",get(job).delete(job_cancel))
        .route("/api/library/move",post(library_move))
        .route("/api/second-copy/run",post(second_copy))
        .route("/api/storage/scrub",post(scrub))
        .route("/api/alerts/{id}/ack",post(alert_ack))
        .route("/api/network",get(network))
        .route("/api/remote/status",get(remote_status))
        .route("/api/screen/sessions",get(screen_sessions))
        .route("/api/screen/sessions/{id}",delete(screen_stop))
        .route("/api/update/stage",post(update_stage))
}

async fn update_stage(State(st):State<DashboardState>)->AdminResult<Json<Value>> {
    let db=st.app.db.clone();let data=st.app.cfg.data_dir.clone();
    let staged=tokio::task::spawn_blocking(move||crate::update::stage(&db,hh_core::HUB_VERSION,&data)).await.map_err(|e|Error::Internal(e.to_string()))??;
    Ok(Json(json!(staged)))
}

async fn session(State(st): State<DashboardState>, headers: HeaderMap) -> AdminResult<Response> {
    if headers.get("x-hh-local").and_then(|v|v.to_str().ok()) != Some(st.token.as_str()) { return Err(Error::Unauthenticated.into()); }
    let cookie = format!("hh_local={}; Path=/; HttpOnly; SameSite=Strict",st.token);
    Ok(([(header::SET_COOKIE,cookie)],Json(json!({"csrf_token":st.token}))).into_response())
}
async fn settings(State(st): State<DashboardState>) -> AdminResult<Json<Value>> {
    let db = &st.app.db;
    let (_,name) = db.hub_identity()?;
    Ok(Json(json!({"hub_name":name,"library_root":st.app.cfg.library_root,"second_copy_root":db.get_setting("second_copy.root")?.or_else(||st.app.cfg.second_copy_root.as_ref().map(|p|p.to_string_lossy().into_owned())),"pause_sharing":db.get_setting("sharing.paused")?.as_deref()==Some("true"),"remote_enabled":db.get_setting("remote.enabled")?.map(|v|v=="true").unwrap_or(st.app.cfg.features.remote),"setup_complete":db.get_setting("setup.complete")?.as_deref()==Some("true"),"screen_enabled":db.get_setting("screen.enabled")?.map(|v|v=="true").unwrap_or(st.app.cfg.features.screen),"update_enabled":db.get_setting("update.enabled")?.as_deref()==Some("true"),"update_manifest_url":db.get_setting("update.manifest_url")?,"update_pubkey_hex":db.get_setting("update.pubkey_hex")?,"backup_interval_minutes":setting_number(db,"backup.interval_minutes",60),"scrub_interval_hours":setting_number(db,"scrub.interval_hours",24),"second_copy_interval_minutes":setting_number(db,"second_copy.interval_minutes",1440),"wake_enabled":db.get_setting("wake.enabled")?.as_deref()==Some("true"),"wake_time":db.get_setting("wake.time")?.unwrap_or_else(||"03:00".into()),"telemetry_opt_in":false})))
}
#[derive(Deserialize)]
struct SettingsPatch {
    hub_name: Option<String>,
    second_copy_root: Option<String>,
    pause_sharing: Option<bool>,
    remote_enabled: Option<bool>,
    setup_complete: Option<bool>,
    screen_enabled:Option<bool>,update_enabled:Option<bool>,
    update_manifest_url:Option<String>,update_pubkey_hex:Option<String>,
    backup_interval_minutes:Option<u64>,scrub_interval_hours:Option<u64>,second_copy_interval_minutes:Option<u64>,
    wake_enabled:Option<bool>,wake_time:Option<String>,
}
async fn save_settings(State(st): State<DashboardState>, Json(req): Json<SettingsPatch>) -> AdminResult<Json<Value>> {
    for (name,value,max) in [("backup interval",req.backup_interval_minutes,10080),("scrub interval",req.scrub_interval_hours,720),("second-copy interval",req.second_copy_interval_minutes,43200)] {if value.is_some_and(|v|v==0||v>max){return Err(Error::BadRequest(format!("invalid {name}")).into());}}
    if let Some(url)=&req.update_manifest_url {if !url.is_empty(){crate::update::validate_https(url)?;}}
    if req.update_pubkey_hex.as_ref().is_some_and(|k|!k.is_empty()&&(k.len()!=64||!k.bytes().all(|v|v.is_ascii_hexdigit()))) {return Err(Error::BadRequest("invalid update public key".into()).into());}
    if let Some(time)=&req.wake_time {hh_hw::wake::validate_time(time)?;}
    if let Some(name) = req.hub_name { st.app.db.set_hub_name(&hh_core::paths::sanitize_component(&name)?)?; }
    if let Some(path) = req.second_copy_root {
        if !path.is_empty() {
            let target = std::path::Path::new(&path);
            if !target.is_absolute() { return Err(Error::BadRequest("second-copy folder must be absolute".into()).into()); }
            std::fs::create_dir_all(target).map_err(Error::from)?;
            let target = target.canonicalize().map_err(Error::from)?;
            let library = st.app.cfg.library_root.canonicalize().map_err(Error::from)?;
            if target.starts_with(&library) || library.starts_with(&target) { return Err(Error::BadRequest("second-copy folder overlaps library".into()).into()); }
            st.app.db.set_setting("second_copy.root",&target.to_string_lossy())?;
        } else { st.app.db.set_setting("second_copy.root","")?; }
    }
    if let Some(value) = req.pause_sharing { set_pause(&st,value).await?; }
    if let Some(value) = req.remote_enabled { st.app.db.set_setting("remote.enabled",if value {"true"} else {"false"})?; }
    if let Some(value) = req.setup_complete { st.app.db.set_setting("setup.complete",if value {"true"} else {"false"})?; }
    for (key,value) in [("screen.enabled",req.screen_enabled),("update.enabled",req.update_enabled)] {if let Some(v)=value{st.app.db.set_setting(key,if v{"true"}else{"false"})?;}}
    for (key,value) in [("backup.interval_minutes",req.backup_interval_minutes),("scrub.interval_hours",req.scrub_interval_hours),("second_copy.interval_minutes",req.second_copy_interval_minutes)] {if let Some(v)=value{st.app.db.set_setting(key,&v.to_string())?;}}
    if let Some(v)=req.update_manifest_url {st.app.db.set_setting("update.manifest_url",&v)?;}
    if let Some(v)=req.update_pubkey_hex {st.app.db.set_setting("update.pubkey_hex",&v)?;}
    if req.wake_enabled.is_some()||req.wake_time.is_some(){let enabled=req.wake_enabled.unwrap_or(st.app.db.get_setting("wake.enabled")?.as_deref()==Some("true"));let time=req.wake_time.unwrap_or(st.app.db.get_setting("wake.time")?.unwrap_or_else(||"03:00".into()));let saved_time=time.clone();let executable=std::env::current_exe().map_err(Error::from)?;tokio::task::spawn_blocking(move||hh_hw::wake::configure(enabled,&time,&executable)).await.map_err(|e|Error::Internal(e.to_string()))??;st.app.db.set_setting("wake.enabled",if enabled{"true"}else{"false"})?;st.app.db.set_setting("wake.time",&saved_time)?;}
    st.app.db.audit(None,"settings_updated",None,None)?;
    settings(State(st)).await
}
async fn set_pause(st:&DashboardState,value:bool)->Result<()> {
    if !value {let c=st.app.db.lock()?;let changing:i64=c.query_row("SELECT COUNT(*) FROM jobs WHERE kind='library_move' AND status IN ('queued','running')",[],|r|r.get(0)).map_err(|e|Error::Db(e.to_string()))?;let path:Option<String>=c.query_row("SELECT path FROM storage_roots WHERE kind='library' AND is_active=1 LIMIT 1",[],|r|r.get(0)).ok();if changing>0||path.is_some_and(|p|std::path::PathBuf::from(p)!=st.app.cfg.library_dir()){return Err(Error::Conflict("restart Home Hub after finishing the library move".into()));}}
    st.app.db.set_setting("sharing.paused",if value {"true"} else {"false"})?;
    if value {
        st.app.pairing.close_window();
        let app = st.app.clone();
        tokio::task::spawn_blocking(move || {
            app.transfers.pause_barrier()?;
            for session in app.stream.active_sessions() {
                if let Err(error) = app.stream.stop_session(&session.id) {
                    tracing::warn!(session_id = %session.id, %error, "screen stop during pause failed");
                }
            }
            if let Some(helper) = &app.helper {
                if let Err(error) = helper.screen_stop() { tracing::warn!(%error, "helper screen stop during pause failed"); }
                if let Err(error) = helper.release_input() { tracing::warn!(%error, "input release during pause failed"); }
            }
            app.screen_owners.lock().map_err(|_|Error::Internal("screen mutex poisoned".into()))?.clear();
            Ok::<(), Error>(())
        }).await.map_err(|e|Error::Internal(e.to_string()))??;
    }
    st.app.events.emit("sharing.changed",json!({"paused":value}));
    st.app.db.audit(None,"sharing_pause",Some(if value {"true"} else {"false"}),None)
}
#[derive(Deserialize)] struct PauseReq { paused:bool }
async fn pause(State(st):State<DashboardState>,Json(req):Json<PauseReq>)->AdminResult<StatusCode>{set_pause(&st,req.paused).await?;Ok(StatusCode::NO_CONTENT)}
async fn pair_pending(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(json!({"pending":if st.app.pairing.is_open(){st.app.pairing.pending()?}else{None}})))}
#[derive(Deserialize)] struct PairConfirm {request_id:String,allow:bool}
async fn pair_confirm(State(st):State<DashboardState>,Json(req):Json<PairConfirm>)->AdminResult<StatusCode>{st.app.pairing.confirm(&req.request_id,req.allow)?;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)] struct DevicePatch{name:Option<String>,scopes:Option<Vec<String>>}
async fn device_patch(State(st):State<DashboardState>,Path(id):Path<String>,Json(req):Json<DevicePatch>)->AdminResult<StatusCode>{
    st.app.db.device_by_id(&id)?.ok_or_else(||Error::NotFound("device".into()))?;
    let name = req.name.as_deref().map(hh_core::paths::sanitize_component).transpose()?;
    if req.scopes.as_ref().is_some_and(|scopes|scopes.len()>5 || scopes.iter().any(|s|!["files","transfer","photos","remote","admin"].contains(&s.as_str()))){return Err(Error::BadRequest("invalid scopes".into()).into());}
    let c=st.app.db.lock()?;
    if let Some(name)=name {c.execute("UPDATE devices SET name=?2 WHERE id=?1",rusqlite::params![id,name]).map_err(|e|Error::Db(e.to_string()))?;}
    if let Some(scopes)=req.scopes {c.execute("UPDATE devices SET scopes=?2 WHERE id=?1",rusqlite::params![id,scopes.join(",")]).map_err(|e|Error::Db(e.to_string()))?;}
    drop(c);st.app.db.audit(None,"device_permissions",Some(&id),None)?;Ok(StatusCode::NO_CONTENT)
}
async fn device_revoke(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{
    let d=st.app.db.device_by_id(&id)?.ok_or_else(||Error::NotFound("device".into()))?;
    st.app.db.revoke_device(&id,"local owner")?;st.app.revocation.revoke(&d.cert_serial);
    let stopping = st.app.clone();
    let revoked_id = id.clone();
    tokio::task::spawn_blocking(move || stopping.stop_device_screens(&revoked_id))
        .await.map_err(|e|Error::Internal(e.to_string()))?;
    st.app.events.emit("device.revoked",json!({"device_id":id}));st.app.db.audit(None,"revoke",Some(&id),None)?;Ok(StatusCode::NO_CONTENT)
}
async fn capabilities(State(st):State<DashboardState>)->AdminResult<Json<Value>>{let helper=st.app.helper.clone();let ping=tokio::task::spawn_blocking(move||helper.and_then(|h|h.ping().ok())).await.map_err(|e|Error::Internal(e.to_string()))?;Ok(Json(json!({"photos":st.app.cfg.features.photos,"remote":st.app.db.get_setting("remote.enabled")?.as_deref()==Some("true")&&ping.is_some(),"screen":ping.as_ref().is_some_and(|v|v["screen"]["available"]==true),"hotspot":ping.as_ref().is_some_and(|v|v["hotspot"]==true),"wol":st.app.cfg.features.wol,"helper_available":ping.is_some(),"hardware":st.app.hw.latest_audit()?})))}
async fn hardware(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(st.app.hw.latest_audit()?.unwrap_or(Value::Null)))}
async fn hardware_run(State(st):State<DashboardState>)->AdminResult<Json<Value>>{let svc=st.app.hw.clone();let result=tokio::task::spawn_blocking(move||svc.run_audit()).await.map_err(|e|Error::Internal(e.to_string()))??;Ok(Json(json!(result)))}
async fn transfer_abort(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{let svc=st.app.transfers.clone();tokio::task::spawn_blocking(move||svc.abort(&id)).await.map_err(|e|Error::Internal(e.to_string()))??;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)] struct NameReq{name:String}
async fn file_rename(State(st):State<DashboardState>,Path(id):Path<String>,Json(req):Json<NameReq>)->AdminResult<StatusCode>{st.app.storage.rename_file(&id,&req.name)?;Ok(StatusCode::NO_CONTENT)}
async fn file_trash(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{st.app.storage.trash_file(&id,None)?;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)] struct MoveReq{rel_path:String,name:Option<String>}
async fn file_move(State(st):State<DashboardState>,Path(id):Path<String>,Json(req):Json<MoveReq>)->AdminResult<StatusCode>{st.app.storage.move_file(&id,&req.rel_path)?;Ok(StatusCode::NO_CONTENT)}
async fn file_copy(State(st):State<DashboardState>,Path(id):Path<String>,Json(req):Json<MoveReq>)->AdminResult<Json<Value>>{let svc=st.app.storage.clone();let result=tokio::task::spawn_blocking(move||svc.copy_file(&id,&req.rel_path,req.name.as_deref())).await.map_err(|e|Error::Internal(e.to_string()))??;Ok(Json(json!(result)))}
async fn trash(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(json!(st.app.storage.list_trash(500)?)))}
async fn restore(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{st.app.storage.restore_file(&id)?;Ok(StatusCode::NO_CONTENT)}
async fn purge(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{st.app.storage.purge_file(&id)?;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)] struct TimelineReq{cursor:Option<String>,limit:Option<u32>}
async fn timeline(State(st):State<DashboardState>,Query(q):Query<TimelineReq>)->AdminResult<Json<Value>>{let cursor=q.cursor.and_then(|v|v.split_once(':').and_then(|(ts,id)|Some((ts.parse().ok()?,id.to_owned()))));let (items,next_cursor)=st.app.photos.timeline(cursor,q.limit.unwrap_or(100).clamp(1,500))?;Ok(Json(json!({"items":items,"next_cursor":next_cursor})))}
async fn years(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(json!(st.app.photos.years()?)))}
async fn duplicates(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(json!(st.app.storage.list_duplicates()?)))}
#[derive(Deserialize)]struct ResolveReq{keep_file_id:String}
async fn duplicate_resolve(State(st):State<DashboardState>,Path(id):Path<String>,Json(req):Json<ResolveReq>)->AdminResult<StatusCode>{st.app.storage.resolve_duplicate_group(&id,&req.keep_file_id)?;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)]struct ImportReq{paths:Vec<String>,mode:Option<String>}
fn validate_import(req:&ImportReq)->Result<()>{if req.paths.is_empty()||req.paths.len()>32{return Err(Error::BadRequest("choose 1 to 32 folders".into()));}Ok(())}
async fn import_scan(State(st):State<DashboardState>,Json(req):Json<ImportReq>)->AdminResult<Json<Value>>{validate_import(&req)?;let svc=st.app.storage.clone();Ok(Json(tokio::task::spawn_blocking(move||svc.scan_import(&req.paths)).await.map_err(|e|Error::Internal(e.to_string()))??))}
async fn import_start(State(st):State<DashboardState>,Json(req):Json<ImportReq>)->AdminResult<Json<Value>>{validate_import(&req)?;Ok(Json(st.app.storage.start_import(req.paths,req.mode.as_deref().unwrap_or("later"))?))}
async fn jobs(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(st.app.storage.list_jobs()?))}
async fn job(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<Json<Value>>{Ok(Json(st.app.storage.job(&id)?))}
async fn job_cancel(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{st.app.storage.cancel_job(&id)?;Ok(StatusCode::NO_CONTENT)}
#[derive(Deserialize)]struct PathReq{path:String}
async fn library_move(State(st):State<DashboardState>,Json(req):Json<PathReq>)->AdminResult<Json<Value>>{
    set_pause(&st,true).await?;
    if !st.app.db.list_transfers(None,Some("open"))?.is_empty()||!st.app.db.list_transfers(None,Some("verifying"))?.is_empty(){return Err(Error::Conflict("finish or cancel transfers before moving library".into()).into());}
    Ok(Json(st.app.storage.start_library_move(&req.path)?))
}
async fn second_copy(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(st.app.storage.start_maintenance("second_copy")?))}
async fn scrub(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(st.app.storage.start_maintenance("scrub")?))}
async fn duplicate_scan(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(st.app.storage.start_maintenance("duplicates")?))}
async fn alert_ack(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode>{let c=st.app.db.lock()?;if c.execute("UPDATE alerts SET acknowledged_at=?2 WHERE id=?1",rusqlite::params![id,hh_core::time::now_ms()]).map_err(|e|Error::Db(e.to_string()))?==0{return Err(Error::NotFound("alert".into()).into());}Ok(StatusCode::NO_CONTENT)}
async fn network(State(st):State<DashboardState>)->AdminResult<Json<Value>>{Ok(Json(json!(st.app.hw.network_info()?)))}

async fn remote_status(State(st):State<DashboardState>)->AdminResult<Json<Value>> {
    let helper=st.app.helper.clone();
    let result=tokio::task::spawn_blocking(move||helper.and_then(|h|h.ping().ok())).await.map_err(|e|Error::Internal(e.to_string()))?;
    Ok(Json(json!({"helper_available":result.is_some(),"helper":result,"enabled":st.app.db.get_setting("remote.enabled")?.as_deref()==Some("true")})))
}
async fn screen_sessions(State(st):State<DashboardState>)->AdminResult<Json<Value>> {Ok(Json(json!({"items":st.app.stream.active_sessions().iter().map(|s|json!({"id":s.id,"device_id":s.device_id,"direction":s.kind,"state":s.state})).collect::<Vec<_>>()})))}
async fn screen_stop(State(st):State<DashboardState>,Path(id):Path<String>)->AdminResult<StatusCode> {let svc=st.app.stream.clone();let sid=id.clone();tokio::task::spawn_blocking(move||svc.stop_session(&sid)).await.map_err(|e|Error::Internal(e.to_string()))??;st.app.screen_owners.lock().map_err(|_|Error::Internal("screen mutex".into()))?.remove(&id);Ok(StatusCode::NO_CONTENT)}

fn setting_number(db:&hh_db::Db,key:&str,default:u64)->u64 {db.get_setting(key).ok().flatten().and_then(|v|v.parse().ok()).unwrap_or(default)}
