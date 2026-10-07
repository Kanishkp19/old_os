# Home Hub — Technical Requirements Document (TRD)

**Version:** 0.1 · **Scope:** Phase 0–3 on Windows; Phase 4 OS notes

---

## 1. Architecture overview

```text
                         HOME NETWORK (LAN)
 ┌────────────────────────────────────────────────────────────┐
 │  Android app ──┐                                           │
 │  macOS app ────┼── HTTPS/HTTP2 (TLS1.3 + mTLS) ──►  HUB    │
 │  iOS app ──────┘        mDNS discovery                      │
 │                                                            │
 │  HUB (Windows service: hh-service)                         │
 │   ├─ hh-net        mDNS, TLS listener, HTTP/2 router       │
 │   ├─ hh-auth       Hub CA, pairing, device certs, revoke   │
 │   ├─ hh-transfer   sessions, chunking, verify, queue sync  │
 │   ├─ hh-storage    library, import, dedupe, SMART, scrub   │
 │   ├─ hh-photos     metadata, thumbs, gallery index         │
 │   ├─ hh-remote     input, media keys, power, WoL registry  │
 │   ├─ hh-stream     (Phase 3) WebRTC capture/encode         │
 │   ├─ hh-db         SQLite (WAL) + migrations               │
 │   └─ dashboard     local web UI (loopback only) + tray     │
 └────────────────────────────────────────────────────────────┘
 Internet: optional, never required for core features.
```

## 2. Technology decisions

| Area | Decision | Rationale |
|---|---|---|
| Hub core | **Rust** (tokio, axum/hyper, rustls) | Low RAM/CPU, memory safety, single static binary, portable to Linux for Phase 4 |
| Alt | Go | Acceptable fallback; faster to write, higher RAM |
| TLS | rustls, TLS 1.3 only, mTLS | No OpenSSL dependency on Windows |
| Transport | HTTP/2 over TLS (MVP); evaluate QUIC (quinn) Phase 2+ | Simple, multiplexed, well-supported by mobile stacks |
| Hashing | **BLAKE3** (chunk + tree root) | Very fast on weak CPUs, built-in Merkle tree |
| Metadata DB | **SQLite** (WAL, `rusqlite` or `sqlx`) | Zero-admin, crash-safe |
| Discovery | mDNS/DNS-SD (`mdns-sd` crate), UDP broadcast fallback | Standard, no typing IPs |
| Windows service | `windows-service` crate | Native SCM integration |
| Tray | `tray-icon` + `tao` (Rust) | No Electron |
| Dashboard | Static Svelte/Vite bundle embedded in binary, served on `127.0.0.1:47801` | Tiny footprint, fast to build |
| Android | Kotlin, Jetpack Compose, WorkManager, OkHttp (H2), CameraX/ML Kit (QR) | Best background support |
| macOS | Swift/SwiftUI, URLSession (H2), Network.framework (mDNS) | Native |
| iOS (Phase 2) | Swift, PHPhotoLibrary, BGProcessingTask | Honest about background limits |
| Installer | WiX (MSI) or Inno Setup; code-signed | Firewall rule, service install |
| Thumbnails | `image` crate + `libvips`/WIC fallback; video via bundled `ffmpeg` (optional) | Keep install small |
| EXIF | `kamadak-exif` / `nom-exif` | Capture date, GPS, camera |
| SMART | WMI `MSFT_PhysicalDisk` + `MSStorageDriver_FailurePredictStatus`; bundled `smartctl` fallback | Broad drive support |

## 3. Resource budget

| Metric | Budget |
|---|---|
| Idle RSS (service) | <80 MB |
| Idle CPU | <1% |
| Transfer CPU (i3-class) | <40% sustained |
| Install size | <60 MB (without ffmpeg) |
| Thumbnail worker | Throttled, idle-priority, paused on battery <20% |

## 4. Ports and network

| Port | Proto | Purpose | Binding |
|---|---|---|---|
| 47800 | TCP (TLS+mTLS, H2) | Hub API for paired devices | LAN interfaces |
| 47801 | TCP (HTTP) | Local dashboard | 127.0.0.1 only |
| 47802 | TCP (TLS, no client cert) | Pairing endpoint (token-gated, active only during pairing window) | LAN, time-boxed |
| 5353 | UDP | mDNS | LAN |
| 47803 | UDP | Broadcast fallback discovery | LAN |

Firewall: Private-profile inbound rules only; never public profile.

### mDNS advertisement
```text
Service: _homehub._tcp.local   Port: 47800
TXT: id=<hub_id>  v=1  name=<friendly>  fp=<sha256(ca_pubkey) first 16 hex>  pair=<0|1>
```

## 5. Pairing protocol (summary; full in API_SPEC.md)

1. Hub generates one-time `pair_token` (128-bit, 5 min TTL), opens pairing window.
2. QR payload: `homehub://pair?h=<hub_id>&t=<token>&fp=<ca_fp>&a=<ip:port,...>&n=<name>`.
3. Client scans, resolves Hub (mDNS or `a=` hints), connects to 47802 pinning CA fingerprint `fp`.
4. Client generates keypair (Ed25519 or P-256, keystore-backed), sends CSR + device name + token.
5. Hub validates token, signs device cert (1 year, renewable), stores device, returns cert + CA cert.
6. Token burned. Client uses mTLS on 47800 thereafter.
7. Revocation: Hub maintains revocation list checked at TLS handshake (custom verifier).

## 6. Transfer engine

### 6.1 Upload (client → Hub)
```text
POST /v1/transfers              {name, size, mime, root_hash?, chunk_size, rel_path, kind, client_item_id}
  → 201 {transfer_id, chunk_size, have: [..bitmap..]}
PUT  /v1/transfers/{id}/chunks/{n}   body=raw bytes, headers: X-Chunk-Hash: blake3-hex
  → 204 | 409 (hash mismatch) | 410 (transfer gone)
GET  /v1/transfers/{id}         → status + verified chunk bitmap (resume)
POST /v1/transfers/{id}/complete {root_hash}
  → 200 {file_id, verified:true}  (Hub recomputes tree root; mismatch → 422)
DELETE /v1/transfers/{id}       → abort, cleanup
```
- Chunk size default **4 MiB** (adaptive 1–8 MiB by link).
- Parallelism: 2–4 concurrent chunk PUTs on H2 streams.
- Hub writes to `<root>/.hh-tmp/<transfer_id>.part` with preallocated size, writes chunk at offset, fsyncs per N chunks and at finalize.
- Finalize: verify tree root → `fsync` → atomic rename to destination → insert `files` row in one SQLite transaction.
- Idempotency: `client_item_id` + `root_hash` dedupe; re-sending existing hash returns existing file (no re-upload).

### 6.2 Download (Hub → client)
`GET /v1/files/{id}/content` with `Range` support; per-chunk hash list via `GET /v1/files/{id}/manifest`.

### 6.3 Resume
Hub persists chunk bitmap (`transfer_chunks`). Client resumes by `GET status`, uploads only missing chunks. Hub verified-chunk state survives restart.

### 6.4 Backup safety rule
`backed_up = true` only after Hub returns `verified:true` for the whole-file root hash **and** client confirms hash equals its local computed hash. "Free phone storage" is gated on this.

### 6.5 Queue-and-send-later (client-side)
- Persistent local queue (Room/SQLite on Android; SwiftData/SQLite on macOS).
- States: `queued → connecting → uploading → verifying → done | failed(retryable) | failed(permanent)`.
- Triggers: Hub discovered via mDNS, network change, app foreground, WorkManager periodic (Android), BGProcessingTask (iOS).
- Exponential backoff with jitter; no battery-hostile polling (use mDNS browse + `NetworkCallback`).
- Source file must remain accessible; if source changes (mtime/size), re-hash.

### 6.6 Throughput estimation
Rolling measurement (EWMA over last 8 chunks) → ETA. Initial estimate from small probe (`GET /v1/ping?bytes=1048576`).

## 7. Storage engine

### 7.1 Library layout (on Windows)
```text
<HubRoot>\                       (default: D:\HomeHub or C:\HomeHubData; user-chosen)
  Library\
    Photos\YYYY\MM\<name>
    Videos\YYYY\MM\<name>
    Documents\
    Music\
    Downloads\
    Backups\<device_name>\
  .hh-tmp\                       partial uploads
  .hh-thumbs\                    thumbnail cache
  hub.db (+ -wal, -shm)          SQLite (kept under %ProgramData%\HomeHub, not in library, by default)
```
Users see logical categories; physical layout is an implementation detail. Existing Windows data is **never moved** unless user chooses Import.

### 7.2 Import
Scanner walks user-selected folders (default: Pictures, Videos, Documents, Downloads), classifies by extension/MIME, reports counts/sizes. Import copies (not moves) by default, hashing as it goes; `source_mode = keep | import`.

### 7.3 Duplicate detection
- Stage 1: size match → BLAKE3 full hash → exact duplicate groups.
- Stage 2 (Phase 2+): pHash/dHash for images; ffmpeg-keyframe hash for videos.
- Never auto-delete. Reclaim = user-approved move to Hub trash (30-day retention).

### 7.4 Health and integrity
- SMART poll every 6 h (and on boot); store snapshots.
- Free-space thresholds: warn 15%, critical 5%.
- Scrub: rolling re-hash of files (e.g., 1–2% of library nightly, idle priority) → `integrity_events`.
- Failure predictions or reallocated-sector growth → `alerts` + push to devices + tray.

### 7.5 Second copy
Target = external drive/path selected by user. Incremental mirror by hash, verified after copy. Reports "last verified copy" timestamp. Phase 5: Hub-to-Hub replication.

## 8. Photo engine
- On upload of image/video: extract EXIF/creation time (fallback mtime), dimensions, duration, GPS (stored, opt-in display).
- Generate thumbs 256px and 1024px (WebP/JPEG); video poster frame.
- Gallery API paginated by month; stable cursor sort `(taken_at DESC, id)`.
- Live Photos / HEIC: store original; thumbnails via Windows WIC HEIF extension if present else skip with placeholder.
- RAW and sidecars preserved as-is.

## 9. Remote engine (Windows)
| Capability | Mechanism |
|---|---|
| Mouse/keyboard | `SendInput` (Win32) via service → user-session helper process (service runs in Session 0, cannot inject into user desktop) |
| Media keys | `SendInput` with `VK_MEDIA_*`, `VK_VOLUME_*` |
| Power | Sleep: `SetSuspendState`; Restart/Shutdown: `InitiateSystemShutdownEx` |
| Wake | Client sends WoL magic packet to Hub MAC (Hub shares MAC list on pair); only works if NIC/BIOS support; Wi-Fi WoWLAN unreliable |

**Important:** Windows services run in Session 0. A per-user **helper process** (`hh-session.exe`, launched at logon) handles input injection, screen capture and tray, and talks to the service via a named pipe with ACLs.

Remote input is rate-limited, requires `remote` permission scope on the device, and shows a tray indicator while active.

## 10. Screen sharing (Phase 3)
- Capture: Windows Graphics Capture / DXGI Desktop Duplication in helper process.
- Encode: hardware (NVENC/QuickSync/AMF via Media Foundation) → H.264; software x264 fallback only if audit says CPU adequate.
- Transport: WebRTC (`webrtc-rs`) on LAN, signaling over existing mTLS API; no STUN/TURN needed.
- Phone → Laptop cast: WebRTC receive in helper, fullscreen window.
- Quality presets: Low (720p30, 2 Mbps), Balanced (1080p30, 6 Mbps), High (1080p60, 12 Mbps), chosen by audit.

## 11. Hardware audit

Collected via WMI/Win32: CPU model/cores/AVX, RAM, disks (type, size, SMART), network adapters (Wi-Fi standard, Ethernet speed), battery health, camera presence, GPU/encoders.

Rating rules (initial):

| Capability | Excellent | Good | Limited | Not recommended |
|---|---|---|---|---|
| Storage | SSD or healthy HDD ≥250 GB | HDD ≥120 GB | <120 GB or SMART caution | SMART failing |
| Photo backup | Wired or Wi-Fi 5/6 | 802.11n 5 GHz | 2.4 GHz only | — |
| File sharing | Any ≥ Good net | — | 2.4 GHz | — |
| Screen streaming | HW encoder + Wi-Fi 5/6 | SW 720p, ≥4 cores | Dual-core | <4 GB RAM, no HW encoder |
| Local AI | ≥16 GB RAM + GPU/NPU | — | — | otherwise |

Results saved to `hardware_audit`; feature flags derive from ratings.

## 12. Update mechanism
- Signed update manifests (Ed25519 key pinned in binary), HTTPS fetch from update host; **opt-in/auto-check toggle**, never required.
- MSI patch or in-place binary swap with service restart; keep previous version for rollback.
- Offline update by installer download.

## 13. Observability
- Structured logs (`tracing`) to `%ProgramData%\HomeHub\logs`, size-rotated (10 × 5 MB).
- Local diagnostics bundle export (user-initiated); **no telemetry by default**.
- Metrics (in-memory, exposed at dashboard): CPU/RAM of service, transfer throughput, queue depth, error counts.

## 14. Error handling
Typed error codes (see API_SPEC). Principles: fail closed on auth, fail safe on storage (never delete on error), partial files never visible in library, all multi-step state transitions in SQLite transactions.

## 15. Phase 4 — Home Hub OS notes
- Base: minimal Linux (Buildroot/Yocto or Debian-minimal/Alpine) + same Rust services (`hh-service` already cross-platform behind OS abstraction traits: `Power`, `Input`, `Capture`, `Smart`, `Service`).
- Shell: kiosk (cage/Wayland compositor) running Tauri/WebView or native shell.
- Updates: A/B image partitions + rollback.
- Installer: USB image, hardware audit, keep/migrate/erase data flow.
- Therefore **keep OS-specific code behind traits from day one.**

## 16. Platform abstraction traits (Rust)

```rust
pub trait PowerControl { fn sleep(&self)->Result<()>; fn restart(&self)->Result<()>; fn shutdown(&self)->Result<()>; }
pub trait InputControl { fn mouse_move(&self,dx:i32,dy:i32)->Result<()>; fn click(&self,btn:Button)->Result<()>; fn key(&self,k:Key,down:bool)->Result<()>; }
pub trait DiskHealth  { fn list(&self)->Result<Vec<DiskInfo>>; fn smart(&self,id:&str)->Result<SmartReport>; }
pub trait HwAudit     { fn collect(&self)->Result<HardwareReport>; }
pub trait ServiceHost { fn run(self)->Result<()>; }
```

## 17. Constraints and decisions log

| # | Decision |
|---|---|
| D1 | Windows first, no Linux for prototype |
| D2 | LAN-only by default; no inbound internet exposure |
| D3 | Queue is the guarantee; wake is best-effort |
| D4 | Android + macOS first; iOS Phase 2 with honest limits |
| D5 | One laptop = one storage pool until Phase 5 |
| D6 | Existing Windows data never auto-modified |
| D7 | BLAKE3 for all integrity checks |
| D8 | No Electron for resident components |
