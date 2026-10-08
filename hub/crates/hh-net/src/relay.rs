//! Recipient-owned, durable staged delivery. Only verified recipient content
//! acknowledges success; library content remains pinned while pending.
use axum::{Json,Router,extract::{State,Path,Extension},routing::{get,post}};
use hh_core::{Error,Result};use hh_core::time::now_ms;
use serde::Deserialize;use serde_json::{json,Value};use rusqlite::{params,OptionalExtension};
use crate::AppState;use crate::routes::PeerIdentityExt;use crate::routes::{ApiErr,require_scope,ident};
type ApiResult<T>=std::result::Result<T,ApiErr>;
pub fn router()->Router<AppState>{Router::new().route("/v1/relay/devices",get(targets)).route("/v1/relay/inbox",get(inbox)).route("/v1/files/{id}/relay",post(stage)).route("/v1/relay/{id}/delivered",post(delivered)).route("/v1/relay/{id}",axum::routing::delete(cancel))}
#[derive(Deserialize)]struct StageReq{target_device_id:String}
async fn stage(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>,Json(req):Json<StageReq>)->ApiResult<Json<Value>> {
    let peer=ident(ext)?;require_scope(&peer,"transfer")?;
    let origin:Option<String>={let c=s.db.lock()?;c.query_row("SELECT origin_device_id FROM files WHERE id=?1 AND deleted_at IS NULL",params![id],|r|r.get(0)).map_err(|e|Error::Db(e.to_string()))?};
    if origin.as_deref()!=Some(&peer.device_id) && !peer.scopes.iter().any(|v|v=="admin"){return Err(Error::ForbiddenScope("relay source".into()).into());}
    Ok(Json(stage_file(&s,&id,&peer.device_id,&req.target_device_id,None)?))
}
pub fn stage_file(s:&AppState,file_id:&str,source:&str,target:&str,transfer:Option<&str>)->Result<Value>{
    let recipient=s.db.device_by_id(target)?.ok_or_else(||Error::NotFound("recipient".into()))?;
    if recipient.status!="active" || !recipient.has_scope("files"){return Err(Error::ForbiddenScope("recipient files".into()));}
    let file=s.storage.get_file(file_id)?;
    let c=s.db.lock()?;
    let active:i64=c.query_row("SELECT COUNT(*) FROM files WHERE id=?1 AND deleted_at IS NULL",params![file_id],|r|r.get(0)).map_err(|e|Error::Db(e.to_string()))?;
    if active!=1{return Err(Error::Conflict("relay file is unavailable".into()));}
    if let Some(tid)=transfer {if let Some((id,status))=c.query_row("SELECT id,status FROM relay_delivery WHERE transfer_id=?1",params![tid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|Error::Db(e.to_string()))?{return Ok(json!({"id":id,"status":status}));}}
    if let Some((id,status))=c.query_row("SELECT id,status FROM relay_delivery WHERE file_id=?1 AND source_device_id=?2 AND target_device_id=?3 AND transfer_id IS NULL AND status IN ('pending','delivered') ORDER BY created_at DESC LIMIT 1",params![file_id,source,target],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|Error::Db(e.to_string()))? {return Ok(json!({"id":id,"status":status}));}
    let id=ulid::Ulid::new().to_string();
    c.execute("INSERT INTO relay_delivery(id,file_id,source_device_id,target_device_id,transfer_id,hash,size,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,file_id,source,target,transfer,file.hash,file.size as i64,now_ms()]).map_err(|e|Error::Db(e.to_string()))?;
    Ok(json!({"id":id,"status":"pending"}))
}
async fn inbox(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>)->ApiResult<Json<Value>>{
    let peer=ident(ext)?;require_scope(&peer,"files")?;let c=s.db.lock()?;
    let mut st=c.prepare("SELECT d.id,d.file_id,f.name,d.size,d.hash,d.source_device_id,d.created_at,d.status FROM relay_delivery d JOIN files f ON f.id=d.file_id WHERE d.target_device_id=?1 AND d.status='pending' AND f.deleted_at IS NULL ORDER BY d.created_at,d.id LIMIT 500").map_err(|e|Error::Db(e.to_string()))?;
    let rows=st.query_map(params![peer.device_id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"file_id":r.get::<_,String>(1)?,"name":r.get::<_,String>(2)?,"size":r.get::<_,i64>(3)?,"hash":r.get::<_,String>(4)?,"source_device_id":r.get::<_,String>(5)?,"created_at":r.get::<_,i64>(6)?,"status":r.get::<_,String>(7)?}))).map_err(|e|Error::Db(e.to_string()))?;
    let items=rows.collect::<std::result::Result<Vec<_>,_>>().map_err(|e|Error::Db(e.to_string()))?;
    Ok(Json(json!({"items":items,"next_cursor":null})))
}
#[derive(Deserialize)]struct Ack{hash:String}
async fn delivered(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>,Json(req):Json<Ack>)->ApiResult<Json<Value>>{
    let peer=ident(ext)?;require_scope(&peer,"files")?;let c=s.db.lock()?;
    let (owner,hash,status):(String,String,String)=c.query_row("SELECT target_device_id,hash,status FROM relay_delivery WHERE id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|Error::NotFound("delivery".into()))?;
    if owner!=peer.device_id {return Err(Error::ForbiddenScope("recipient".into()).into());}
    if status=="cancelled" || !hash.eq_ignore_ascii_case(&req.hash){return Err(Error::Conflict("delivery hash or state".into()).into());}
    c.execute("UPDATE relay_delivery SET status='delivered',delivered_at=COALESCE(delivered_at,?2) WHERE id=?1",params![id,now_ms()]).map_err(|e|Error::Db(e.to_string()))?;
    Ok(Json(json!({"delivered":true})))
}
async fn cancel(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>,Path(id):Path<String>)->ApiResult<Json<Value>>{
    let peer=ident(ext)?;require_scope(&peer,"transfer")?;let c=s.db.lock()?;
    let(owner,target):(String,String)=c.query_row("SELECT source_device_id,target_device_id FROM relay_delivery WHERE id=?1",params![id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|Error::NotFound("delivery".into()))?;
    if owner!=peer.device_id&&target!=peer.device_id{return Err(Error::ForbiddenScope("delivery owner".into()).into());}
    c.execute("UPDATE relay_delivery SET status='cancelled' WHERE id=?1 AND status='pending'",params![id]).map_err(|e|Error::Db(e.to_string()))?;Ok(Json(json!({"cancelled":true})))
}

async fn targets(State(s):State<AppState>,ext:Option<Extension<PeerIdentityExt>>)->ApiResult<Json<Value>> {
    let peer=ident(ext)?;require_scope(&peer,"transfer")?;
    let items=s.db.list_devices()?.into_iter().filter(|d|d.status=="active"&&d.id!=peer.device_id&&d.has_scope("files")).take(500).map(|d|json!({"id":d.id,"name":d.name,"platform":d.platform})).collect::<Vec<_>>();
    Ok(Json(json!({"items":items})))
}
