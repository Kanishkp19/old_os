//! Shared API types (API_SPEC v1). Serde field names match the wire format.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub details: serde_json::Value,
}

impl ApiError {
    pub fn from_err(e: &crate::error::Error) -> Self {
        Self {
            error: ApiErrorBody {
                code: e.code().to_string(),
                message: e.to_string(),
                retryable: e.retryable(),
                details: serde_json::Value::Null,
            },
        }
    }
}

// ---- Hub & device info (API_SPEC §4) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubInfo {
    pub hub_id: String,
    pub name: String,
    pub version: String,
    pub api_min: u32,
    pub api_max: u32,
    pub features: FeaturesWire,
    pub network: NetworkWire,
    pub time: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeaturesWire {
    pub photos: bool,
    pub remote: bool,
    pub screen: bool,
    pub hotspot: bool,
    pub wol: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkWire {
    pub kind: String, // wifi | ethernet | hotspot | unknown
    pub link_mbps: Option<u32>,
    pub wifi_standard: Option<String>,
    /// Internet reachability (W2.9): Some(false) means "no internet but LAN
    /// still works" — Home Hub is LAN-first and this is NOT an error state.
    #[serde(default)]
    pub internet: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub model: Option<String>,
    pub app_version: Option<String>,
    pub scopes: Vec<String>,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
    pub status: String,
}

// ---- Pairing (API_SPEC §3) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequest {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    pub device_name: String,
    pub platform: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
    pub csr_pem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResponse {
    pub device_id: String,
    pub cert_pem: String,
    pub ca_cert_pem: String,
    pub cert_expires_at: i64,
    pub scopes: Vec<String>,
    pub hub: PairHubInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairHubInfo {
    pub id: String,
    pub name: String,
    pub api: u32,
}

// ---- Transfers (API_SPEC §5) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferRequest {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String, // send | backup
    #[serde(default)]
    pub rel_path: Option<String>,
    #[serde(default)]
    pub chunk_size: Option<u64>,
    #[serde(default)]
    pub client_item_id: Option<String>,
    #[serde(default)]
    pub root_hash: Option<String>,
    #[serde(default)]
    pub taken_at: Option<i64>,
    #[serde(default)]
    pub target_device_id: Option<String>,
}

fn default_kind() -> String {
    "send".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferResponse {
    pub transfer_id: String,
    pub chunk_size: u64,
    pub chunk_count: u64,
    pub have: ChunkBitmap,
    pub already_exists: bool,
    pub existing_file_id: Option<String>,
}

/// Verified-chunk bitmap, encoded as inclusive ranges [[start,end], ...].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkBitmap {
    pub encoding: String, // always "ranges"
    pub ranges: Vec<(u64, u64)>,
}

impl ChunkBitmap {
    pub fn from_chunks(mut chunks: Vec<u64>) -> Self {
        chunks.sort_unstable();
        let mut ranges: Vec<(u64, u64)> = Vec::new();
        for c in chunks {
            if let Some(last) = ranges.last_mut() {
                if c == last.1 + 1 {
                    last.1 = c;
                    continue;
                }
            }
            ranges.push((c, c));
        }
        Self { encoding: "ranges".into(), ranges }
    }

    pub fn contains(&self, idx: u64) -> bool {
        self.ranges.iter().any(|(a, b)| idx >= *a && idx <= *b)
    }

    pub fn count(&self) -> u64 {
        self.ranges.iter().map(|(a, b)| b - a + 1).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStatus {
    pub transfer_id: String,
    pub status: String,
    pub name: String,
    pub size: u64,
    pub bytes_verified: u64,
    pub chunk_size: u64,
    pub chunk_count: u64,
    pub have: ChunkBitmap,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_chunks: Vec<u64>,
    pub error_code: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteRequest {
    pub root_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteResponse {
    pub file_id: String,
    pub verified: bool,
    pub hash: String,
    pub size: u64,
    pub rel_path: String,
}

// ---- Files (API_SPEC §6) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObject {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mime: Option<String>,
    pub size: u64,
    pub hash: String,
    pub created_at: i64,
    pub modified_at: Option<i64>,
    pub path: String,
    pub last_verified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

// ---- Status / events ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubStatus {
    pub online: bool,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub active_transfers: u32,
    pub alerts_count: u32,
    pub health_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubEvent {
    pub event: String, // transfer.progress | transfer.completed | alert.created | ...
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_ranges() {
        let bm = ChunkBitmap::from_chunks(vec![0, 1, 2, 4, 5, 9]);
        assert_eq!(bm.ranges, vec![(0, 2), (4, 5), (9, 9)]);
        assert!(bm.contains(4));
        assert!(!bm.contains(3));
        assert_eq!(bm.count(), 6);
    }

    #[test]
    fn bitmap_empty() {
        let bm = ChunkBitmap::from_chunks(vec![]);
        assert_eq!(bm.encoding, "ranges");
        assert_eq!(bm.count(), 0);
    }
}
