//! Versioned, bounded messages for the owner-only local broker.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

pub const LOCAL_IPC_VERSION: u32 = 1;
pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalIpcRequest {
    pub id: String,
    pub op: LocalOp,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalIpcResponse {
    pub id: String,
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<LocalIpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum LocalIpcError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalOp {
    ShellSnapshot,
    AppLaunch,
    NotificationsList,
    NotificationDismiss,
    NotificationClearAll,
    QuickSettingsSet,
    AlbumsList,
    AlbumsCreate,
    AlbumsUpdate,
    AlbumsDelete,
    AlbumsItemsList,
    AlbumsItemsAdd,
    AlbumsItemsRemove,
    BackupSummary,
    NetworkList,
    NetworkConnect,
    NetworkDisconnect,
    AudioSet,
    DisplaySet,
    PowerRequest,
    OwnerUnlock,
    OwnerLock,
    PreferencesGet,
    PreferencesSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppLaunchReq {
    pub app_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationRecord {
    pub id: String,
    pub title: String,
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: i64,
    pub app_id: Option<String>,
    pub is_dismissed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationDismissReq {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct QuickSettingsUpdate {
    pub wifi_enabled: Option<bool>,
    pub volume_pct: Option<u8>,
    pub is_muted: Option<bool>,
    pub brightness_pct: Option<u8>,
    pub dnd_active: Option<bool>,
}

/// Encode one JSON message with a four-byte big-endian length prefix.
pub fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, LocalIpcError> {
    let body = serde_json::to_vec(message)
        .map_err(|e| LocalIpcError::InvalidInput(format!("JSON encode: {e}")))?;
    if body.len() > MAX_FRAME_SIZE {
        return Err(LocalIpcError::InvalidInput("frame exceeds 1 MiB".into()));
    }
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Decode exactly one complete frame; callers must bound reads before allocation.
pub fn decode_frame<T: DeserializeOwned>(frame: &[u8]) -> Result<T, LocalIpcError> {
    let prefix: [u8; 4] = frame
        .get(..4)
        .ok_or_else(|| LocalIpcError::InvalidInput("short frame header".into()))?
        .try_into()
        .map_err(|_| LocalIpcError::InvalidInput("short frame header".into()))?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size > MAX_FRAME_SIZE || frame.len() != size + 4 {
        return Err(LocalIpcError::InvalidInput("invalid frame length".into()));
    }
    serde_json::from_slice(&frame[4..])
        .map_err(|e| LocalIpcError::InvalidInput(format!("JSON decode: {e}")))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumRecord {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub item_count: u32,
    pub cover_file_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumItemRecord {
    pub album_id: String,
    pub file_id: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsListQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsCreateReq {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsUpdateReq {
    pub id: String,
    pub expected_name: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsDeleteReq {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsItemsListQuery {
    pub id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsItemsAddReq {
    pub id: String,
    pub file_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumsItemsRemoveReq {
    pub id: String,
    pub file_ids: Vec<String>,
}

fn validate_ulid(id: &str) -> Result<(), LocalIpcError> {
    id.parse::<ulid::Ulid>()
        .map(|_| ())
        .map_err(|_| LocalIpcError::InvalidInput("identifier must be a ULID".into()))
}

fn validate_file_batch(id: &str, file_ids: &[String]) -> Result<(), LocalIpcError> {
    validate_ulid(id)?;
    if file_ids.is_empty() || file_ids.len() > 100 {
        return Err(LocalIpcError::InvalidInput(
            "album batch must contain 1–100 file IDs".into(),
        ));
    }
    let mut unique = HashSet::with_capacity(file_ids.len());
    for file_id in file_ids {
        validate_ulid(file_id)?;
        if !unique.insert(file_id) {
            return Err(LocalIpcError::InvalidInput(
                "duplicate file ID in album batch".into(),
            ));
        }
    }
    Ok(())
}

impl AlbumsItemsAddReq {
    pub fn validate(&self) -> Result<(), LocalIpcError> {
        validate_file_batch(&self.id, &self.file_ids)
    }
}

impl AlbumsItemsRemoveReq {
    pub fn validate(&self) -> Result<(), LocalIpcError> {
        validate_file_batch(&self.id, &self.file_ids)
    }
}

pub fn validate_album_name(name: &str) -> Result<String, LocalIpcError> {
    if name.chars().any(char::is_control) {
        return Err(LocalIpcError::InvalidInput(
            "album name contains control characters".into(),
        ));
    }
    let normalized: String = name.trim().nfc().collect();
    let characters = normalized.chars().count();
    if characters == 0
        || characters > 120
        || normalized.len() > 480
        || normalized.chars().any(char::is_control)
    {
        return Err(LocalIpcError::InvalidInput(
            "album name must be 1–120 non-control characters and at most 480 UTF-8 bytes".into(),
        ));
    }
    Ok(normalized)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub timestamp: i64,
    pub hub_name: String,
    pub storage_available_bytes: u64,
    pub storage_healthy: bool,
    pub connected_devices: u32,
    pub wifi_ssid: Option<String>,
    pub volume_pct: u8,
    pub battery_pct: Option<u8>,
    pub is_charging: bool,
    pub is_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupSummary {
    pub total_items: u64,
    pub verified_items: u64,
    pub pending_items: u64,
    pub last_backup_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_dtos_round_trip_json_and_frames() {
        let launch = AppLaunchReq {
            app_id: "photos".into(),
        };
        let notice = NotificationRecord {
            id: "n1".into(),
            title: "Disk".into(),
            message: "Needs attention".into(),
            level: NotificationLevel::Critical,
            timestamp: 42,
            app_id: Some("files".into()),
            is_dismissed: false,
        };
        assert_eq!(
            serde_json::from_value::<AppLaunchReq>(serde_json::to_value(&launch).unwrap()).unwrap(),
            launch
        );
        assert_eq!(
            serde_json::from_value::<NotificationRecord>(serde_json::to_value(&notice).unwrap())
                .unwrap(),
            notice
        );
        assert_eq!(
            decode_frame::<AppLaunchReq>(&encode_frame(&launch).unwrap()).unwrap(),
            launch
        );
        assert_eq!(
            decode_frame::<NotificationRecord>(&encode_frame(&notice).unwrap()).unwrap(),
            notice
        );
    }

    #[test]
    fn request_frame_round_trip_and_bounds() {
        let request = LocalIpcRequest {
            id: "request-1".into(),
            op: LocalOp::AlbumsList,
            payload: serde_json::json!({"limit": 25}),
        };
        let frame = encode_frame(&request).unwrap();
        assert_eq!(
            u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize,
            frame.len() - 4
        );
        assert_eq!(decode_frame::<LocalIpcRequest>(&frame).unwrap(), request);
        assert_eq!(
            serde_json::to_value(LocalOp::AlbumsItemsAdd).unwrap(),
            "albums_items_add"
        );
        assert!(decode_frame::<LocalIpcRequest>(&frame[..3]).is_err());
        assert!(decode_frame::<LocalIpcRequest>(&[0, 16, 0, 1]).is_err());
        let mut trailing = frame;
        trailing.push(0);
        assert!(decode_frame::<LocalIpcRequest>(&trailing).is_err());
    }

    #[test]
    fn album_name_normalizes_and_limits() {
        assert_eq!(validate_album_name("  e\u{301}  ").unwrap(), "é");
        for bad in ["", "  ", "a\n", "a\u{0}"] {
            assert!(validate_album_name(bad).is_err());
        }
        assert!(validate_album_name(&"a".repeat(120)).is_ok());
        assert!(validate_album_name(&"a".repeat(121)).is_err());
        assert!(validate_album_name(&"😀".repeat(120)).is_ok());
        assert!(validate_album_name(&format!("{}a", "😀".repeat(120))).is_err());
    }

    #[test]
    fn error_has_stable_display_and_wire_form() {
        let error = LocalIpcError::Conflict("stale album name".into());
        assert_eq!(error.to_string(), "conflict: stale album name");
        let wire = serde_json::to_string(&error).unwrap();
        assert_eq!(serde_json::from_str::<LocalIpcError>(&wire).unwrap(), error);
    }

    #[test]
    fn album_batch_rejects_paths_duplicates_and_oversize() {
        let id = ulid::Ulid::new().to_string();
        let file_id = ulid::Ulid::new().to_string();
        let mut request = AlbumsItemsAddReq {
            id,
            file_ids: vec![file_id.clone()],
        };
        assert!(request.validate().is_ok());
        request.file_ids.push(file_id);
        assert!(request.validate().is_err());
        request.file_ids = vec!["../../etc/passwd".into()];
        assert!(request.validate().is_err());
        request.file_ids = (0..101).map(|_| ulid::Ulid::new().to_string()).collect();
        assert!(request.validate().is_err());
    }
}
