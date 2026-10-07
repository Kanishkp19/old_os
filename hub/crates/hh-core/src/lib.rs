//! hh-core: shared types, errors, config, path safety and platform traits.
//!
//! Everything OS-specific in Home Hub goes behind the traits in [`platform`]
//! so the Phase 4 Linux port (TRD §15) swaps implementations, not call sites.

pub mod config;
pub mod error;
pub mod paths;
pub mod platform;
pub mod time;
pub mod types;

pub use config::Config;
pub use error::{Error, Result};
pub use types::*;

/// Hub API port for paired devices (TLS 1.3 + mTLS, HTTP/2). TRD §4.
pub const API_PORT: u16 = 47800;
/// Loopback-only local dashboard. TRD §4.
pub const DASHBOARD_PORT: u16 = 47801;
/// Token-gated pairing endpoint, open only during a pairing window. TRD §4.
pub const PAIRING_PORT: u16 = 47802;
/// UDP broadcast discovery fallback. TRD §4.
pub const DISCOVERY_PORT: u16 = 47803;
/// Default transfer chunk size (4 MiB). Adaptive 1–8 MiB by link. TRD §6.1.
pub const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
pub const MIN_CHUNK_SIZE: u64 = 1024 * 1024;
pub const MAX_CHUNK_SIZE: u64 = 8 * 1024 * 1024;
/// Pairing token TTL (TRD §5 / SECURITY §5).
pub const PAIR_TOKEN_TTL_MS: i64 = 5 * 60 * 1000;
/// Manual pairing code TTL (API_SPEC §3).
pub const PAIR_CODE_TTL_MS: i64 = 2 * 60 * 1000;
/// Open transfer idle TTL before GC (API_SPEC §11).
pub const TRANSFER_IDLE_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// Trash retention (BACKEND_SCHEMA §11).
pub const TRASH_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Discovery magic for the UDP broadcast fallback (API_SPEC §2).
pub const DISCOVERY_MAGIC: &[u8] = b"HHDISCOVER1";
/// mDNS service type (TRD §4).
pub const MDNS_SERVICE_TYPE: &str = "_homehub._tcp.local.";
/// Current API version.
pub const API_VERSION: u32 = 1;
/// Hub software version (from Cargo).
pub const HUB_VERSION: &str = env!("CARGO_PKG_VERSION");
