use crate::app_registry::verify_caller;
use crate::store::Result;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use tauri::WebviewWindow;
use unicode_normalization::UnicodeNormalization;

fn open_hub_db(state: &crate::AppState) -> Result<Connection> {
    let db_path = state.hub_db_path();
    Connection::open(&db_path).map_err(|e| format!("Cannot open hub database: {e}"))
}

fn validate_id(id: &str) -> Result<()> {
    id.parse::<ulid::Ulid>()
        .map(|_| ())
        .map_err(|_| "Identifier must be a ULID".into())
}

fn validate_name(name: &str) -> Result<String> {
    if name.chars().any(char::is_control) { return Err("Invalid album name".into()); }
    let name: String = name.trim().nfc().collect();
    if name.is_empty() || name.chars().any(char::is_control) || name.chars().count() > 120 || name.len() > 480 {
        return Err("Invalid album name".into());
    }
    Ok(name)
}

fn parse_cursor(value: Option<&str>) -> Result<Option<(i64, String)>> {
    value.map(|value| {
        let (time, id) = value.split_once(':').ok_or("Invalid cursor")?;
        let time = time.parse::<i64>().map_err(|_| "Invalid cursor")?;
        validate_id(id)?;
        Ok((time, id.to_owned()))
    }).transpose()
}

fn album(db: &Connection, id: &str) -> Result<Value> {
    db.query_row(
        "SELECT a.id,a.name,a.created_at,
         (SELECT COUNT(DISTINCT ai.file_id) FROM album_items ai JOIN files f ON f.id=ai.file_id JOIN media m ON m.file_id=f.id WHERE ai.album_id=a.id AND f.deleted_at IS NULL),
         (SELECT ai.file_id FROM album_items ai JOIN files f ON f.id=ai.file_id JOIN media m ON m.file_id=f.id WHERE ai.album_id=a.id AND f.deleted_at IS NULL ORDER BY ai.added_at,ai.file_id LIMIT 1)
         FROM albums a WHERE a.id=?1",
        params![id],
        |row| Ok(json!({"id":row.get::<_,String>(0)?,"name":row.get::<_,String>(1)?,"created_at":row.get::<_,i64>(2)?,"item_count":row.get::<_,i64>(3)?,"cover_file_id":row.get::<_,Option<String>>(4)?})),
    ).optional().map_err(|e| format!("Cannot read album: {e}"))?.ok_or_else(|| "Album not found".into())
}

fn now_ms() -> Result<i64> {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Invalid system clock")?.as_millis();
    i64::try_from(ms).map_err(|_| "Invalid system clock".into())
}

fn validate_file_ids(file_ids: &[String]) -> Result<()> {
    if file_ids.len() > 100 { return Err("Too many file IDs".into()); }
    let mut seen = std::collections::HashSet::new();
    for id in file_ids {
        validate_id(id)?;
        if !seen.insert(id) { return Err("Duplicate file ID".into()); }
    }
    Ok(())
}

#[tauri::command]
pub fn albums_list(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, cursor: Option<String>, limit: Option<u32>) -> Result<Value> {
    verify_caller(&window, "albums_list")?;
    let cursor = parse_cursor(cursor.as_deref())?;
    let limit = limit.unwrap_or(50).clamp(1, 100) as usize;
    let db = open_hub_db(&state)?;
    let mut stmt = db.prepare("SELECT id,created_at FROM albums WHERE (?1 IS NULL OR created_at < ?1 OR (created_at=?1 AND id < ?2)) ORDER BY created_at DESC,id DESC LIMIT ?3")
        .map_err(|e| format!("Cannot list albums: {e}"))?;
    let rows = stmt.query_map(params![cursor.as_ref().map(|v|v.0),cursor.as_ref().map(|v|&v.1),(limit+1) as i64], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))
        .map_err(|e| format!("Cannot list albums: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| format!("Cannot list albums: {e}"))?;
    let more = rows.len() > limit;
    let next_cursor = if more { rows.get(limit-1).map(|(id,time)|format!("{time}:{id}")) } else { None };
    let items = rows.iter().take(limit).map(|(id,_)|album(&db,id)).collect::<Result<Vec<_>>>()?;
    Ok(json!({"items":items,"next_cursor":next_cursor}))
}

#[tauri::command]
pub fn albums_create(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String, name: String) -> Result<Value> {
    verify_caller(&window, "albums_create")?;
    validate_id(&id)?;
    let name = validate_name(&name)?;
    let mut db = open_hub_db(&state)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| format!("Cannot create album: {e}"))?;
    let existing: Option<String> = tx.query_row("SELECT name FROM albums WHERE id=?1",params![id],|r|r.get(0)).optional().map_err(|e| format!("Cannot read album: {e}"))?;
    if let Some(existing) = existing {
        if existing != name { return Err("Album ID conflicts with an existing name".into()); }
    } else {
        tx.execute("INSERT INTO albums(id,name,created_at) VALUES(?1,?2,?3)",params![id,name,now_ms()?]).map_err(|e| format!("Cannot create album: {e}"))?;
    }
    let result = album(&tx,&id)?;
    tx.commit().map_err(|e| format!("Cannot create album: {e}"))?;
    Ok(result)
}

#[tauri::command]
pub fn albums_update(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String, expected_name: String, name: String) -> Result<Value> {
    verify_caller(&window, "albums_update")?;
    validate_id(&id)?;
    let name = validate_name(&name)?;
    let mut db = open_hub_db(&state)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| format!("Cannot update album: {e}"))?;
    let current: Option<String> = tx.query_row("SELECT name FROM albums WHERE id=?1",params![id],|r|r.get(0)).optional().map_err(|e| format!("Cannot read album: {e}"))?;
    if current.as_deref().ok_or("Album not found")? != expected_name { return Err("Album name changed; reload and try again".into()); }
    tx.execute("UPDATE albums SET name=?1 WHERE id=?2",params![name,id]).map_err(|e| format!("Cannot update album: {e}"))?;
    let result = album(&tx,&id)?;
    tx.commit().map_err(|e| format!("Cannot update album: {e}"))?;
    Ok(result)
}

#[tauri::command]
pub fn albums_delete(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String) -> Result<Value> {
    verify_caller(&window, "albums_delete")?;
    validate_id(&id)?;
    let mut db = open_hub_db(&state)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| format!("Cannot delete album: {e}"))?;
    tx.execute("DELETE FROM album_items WHERE album_id=?1",params![id]).map_err(|e| format!("Cannot delete album: {e}"))?;
    tx.execute("DELETE FROM albums WHERE id=?1",params![id]).map_err(|e| format!("Cannot delete album: {e}"))?;
    tx.commit().map_err(|e| format!("Cannot delete album: {e}"))?;
    Ok(json!({"deleted":true}))
}

#[tauri::command]
pub fn albums_items_list(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String, cursor: Option<String>, limit: Option<u32>) -> Result<Value> {
    verify_caller(&window, "albums_items_list")?;
    validate_id(&id)?;
    let cursor = parse_cursor(cursor.as_deref())?;
    let limit = limit.unwrap_or(50).clamp(1,100) as usize;
    let db = open_hub_db(&state)?;
    let _ = album(&db,&id)?;
    let mut stmt = db.prepare("SELECT ai.file_id,ai.added_at FROM album_items ai JOIN files f ON f.id=ai.file_id JOIN media m ON m.file_id=f.id WHERE ai.album_id=?1 AND f.deleted_at IS NULL AND (?2 IS NULL OR ai.added_at > ?2 OR (ai.added_at=?2 AND ai.file_id > ?3)) ORDER BY ai.added_at,ai.file_id LIMIT ?4")
        .map_err(|e| format!("Cannot list album items: {e}"))?;
    let rows = stmt.query_map(params![id,cursor.as_ref().map(|v|v.0),cursor.as_ref().map(|v|&v.1),(limit+1) as i64],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))
        .map_err(|e| format!("Cannot list album items: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| format!("Cannot list album items: {e}"))?;
    let more = rows.len() > limit;
    let next_cursor = if more { rows.get(limit-1).map(|(file_id,time)|format!("{time}:{file_id}")) } else { None };
    let items: Vec<_> = rows.into_iter().take(limit).map(|(file_id,added_at)|json!({"album_id":id,"file_id":file_id,"added_at":added_at})).collect();
    Ok(json!({"items":items,"next_cursor":next_cursor}))
}

#[tauri::command]
pub fn albums_items_add(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String, file_ids: Vec<String>) -> Result<Value> {
    verify_caller(&window, "albums_items_add")?;
    validate_id(&id)?;
    validate_file_ids(&file_ids)?;
    let mut db = open_hub_db(&state)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| format!("Cannot add album items: {e}"))?;
    let _ = album(&tx,&id)?;
    for file_id in &file_ids {
        let visible: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM files f JOIN media m ON m.file_id=f.id WHERE f.id=?1 AND f.deleted_at IS NULL)",params![file_id],|r|r.get(0)).map_err(|e| format!("Cannot read media: {e}"))?;
        if !visible { return Err("Media file not found".into()); }
    }
    let mut added = 0;
    for file_id in &file_ids {
        added += tx.execute("INSERT OR IGNORE INTO album_items(album_id,file_id,added_at) VALUES(?1,?2,?3)",params![id,file_id,now_ms()?]).map_err(|e| format!("Cannot add album items: {e}"))?;
    }
    tx.commit().map_err(|e| format!("Cannot add album items: {e}"))?;
    Ok(json!({"added":added,"already_present":file_ids.len()-added}))
}

#[tauri::command]
pub fn albums_items_remove(window: WebviewWindow, state: tauri::State<'_, crate::AppState>, id: String, file_ids: Vec<String>) -> Result<Value> {
    verify_caller(&window, "albums_items_remove")?;
    validate_id(&id)?;
    validate_file_ids(&file_ids)?;
    let mut db = open_hub_db(&state)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| format!("Cannot remove album items: {e}"))?;
    let _ = album(&tx,&id)?;
    let mut removed = 0;
    for file_id in &file_ids {
        removed += tx.execute("DELETE FROM album_items WHERE album_id=?1 AND file_id=?2",params![id,file_id]).map_err(|e| format!("Cannot remove album items: {e}"))?;
    }
    tx.commit().map_err(|e| format!("Cannot remove album items: {e}"))?;
    Ok(json!({"removed":removed}))
}

#[tauri::command]
pub fn backup_summary(window: WebviewWindow, state: tauri::State<'_, crate::AppState>) -> Result<Value> {
    verify_caller(&window, "backup_summary")?;
    let db = open_hub_db(&state)?;
    db.query_row("SELECT COUNT(*),COUNT(CASE WHEN status='verified' THEN 1 END),COUNT(CASE WHEN status='pending' THEN 1 END),MAX(verified_at) FROM backup_items",[],|r|Ok(json!({"total_items":r.get::<_,i64>(0)?,"verified_items":r.get::<_,i64>(1)?,"pending_items":r.get::<_,i64>(2)?,"last_backup_at":r.get::<_,Option<i64>>(3)?})))
        .map_err(|e| format!("Cannot read backup summary: {e}"))
}
