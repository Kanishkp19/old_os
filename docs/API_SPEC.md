# Home Hub — API Specification (v1)

**Transport:** HTTPS (HTTP/2), TLS 1.3, mutual TLS · **Base:** `https://<hub>:47800/v1` · **Encoding:** JSON (UTF-8) unless noted · **IDs:** ULID · **Time:** epoch ms UTC

---

## 1. Conventions

### Auth
- All endpoints on port 47800 require a valid **client certificate** issued by the Hub CA and not revoked.
- Device **scopes** (from `devices.scopes`): `files`, `transfer`, `photos`, `remote`, `admin`. Endpoint tables list required scope.
- Pairing endpoints (port 47802) are token-gated, TLS without client cert, only active during a pairing window.
- Dashboard (127.0.0.1:47801) is separate, loopback-only, with a local session secret.

### Errors
```json
{ "error": { "code": "HASH_MISMATCH", "message": "Chunk 14 hash mismatch", "retryable": true, "details": {} } }
```

| HTTP | code | Meaning |
|---|---|---|
| 400 | BAD_REQUEST | Malformed input |
| 401 | UNAUTHENTICATED | No/invalid client cert |
| 403 | FORBIDDEN_SCOPE | Missing scope |
| 403 | DEVICE_REVOKED | Cert revoked |
| 404 | NOT_FOUND | Resource missing |
| 409 | HASH_MISMATCH | Chunk hash differs from header |
| 409 | CONFLICT | Name/state conflict |
| 410 | TRANSFER_GONE | Transfer expired/aborted |
| 413 | TOO_LARGE | Exceeds limits |
| 422 | ROOT_HASH_MISMATCH | Whole-file verification failed |
| 423 | PAIRING_LOCKED | Too many token attempts |
| 429 | RATE_LIMITED | Throttled (`Retry-After`) |
| 503 | STORAGE_FULL / STORAGE_UNAVAILABLE | No space / root offline |
| 507 | INSUFFICIENT_STORAGE | Preflight failed |

### Pagination
Cursor-based: `?limit=100&cursor=<opaque>` → `{ "items": [...], "next_cursor": "..." | null }`.

### Versioning
Path prefix `/v1`. `GET /v1/info` returns `api_min`/`api_max`. Clients must tolerate unknown fields.

---

## 2. Discovery

### mDNS
```text
_homehub._tcp.local.  port 47800
TXT: id=<hub_id> v=1 name=<url-encoded friendly name> fp=<16-hex CA fingerprint> pair=<0|1>
```

### UDP broadcast fallback (port 47803)
Client sends `HHDISCOVER1`; Hub replies JSON `{ "id":..., "name":..., "port":47800, "pair_port":47802, "fp":"..." }`.

---

## 3. Pairing (port 47802)

### QR payload
```text
homehub://pair?h=<hub_id>&t=<token_b64url>&fp=<ca_fp_16hex>&a=<ip:port,ip:port>&n=<name>
```
Token: 128-bit random, TTL 5 min, single use, 5 attempts max.

### `POST /pair`
Client must pin CA fingerprint `fp` from QR (verify the Hub's presented chain roots to a CA whose SHA-256 pubkey hash starts with `fp`).

Request:
```json
{
  "token": "…",
  "device_name": "Kanishk's Pixel",
  "platform": "android",
  "model": "Pixel 7",
  "app_version": "0.1.0",
  "csr_pem": "-----BEGIN CERTIFICATE REQUEST-----…"
}
```
Response 200:
```json
{
  "device_id": "01J…",
  "cert_pem": "…",
  "ca_cert_pem": "…",
  "cert_expires_at": 1790000000000,
  "scopes": ["files","transfer","photos"],
  "hub": { "id": "01J…", "name": "Kanishk's Home Hub", "api": 1 }
}
```
Errors: `401 INVALID_TOKEN`, `410 TOKEN_EXPIRED`, `423 PAIRING_LOCKED`.

Hub shows on-screen confirmation "Allow *Kanishk's Pixel*?" in tray/dashboard when `confirm_on_hub` setting is on (default on for non-QR manual code pairing).

### Manual code fallback
Dashboard shows 6-digit code (TTL 2 min). Client calls `POST /pair` with `{"code":"123456", ...}` and **requires** on-Hub confirmation.

---

## 4. Hub and device info

| Method | Path | Scope | Description |
|---|---|---|---|
| GET | `/info` | any | Hub id, name, version, api range, features flags, time |
| GET | `/ping?bytes=N` | any | Returns N bytes (≤8 MiB) for throughput probe |
| GET | `/status` | any | Free space, health summary, active transfers, alerts count |
| GET | `/devices` | admin | List paired devices |
| DELETE | `/devices/{id}` | admin | Revoke device |
| PATCH | `/devices/me` | any | Rename self, update app_version, push token |
| POST | `/certs/renew` | any | CSR → new cert (if <30 days to expiry) |

`GET /info` response:
```json
{
  "hub_id": "01J…", "name": "Kanishk's Home Hub", "version": "0.1.0",
  "api_min": 1, "api_max": 1,
  "features": { "photos": true, "remote": true, "screen": false, "hotspot": false, "wol": true },
  "network": { "kind": "wifi", "link_mbps": 144, "wifi_standard": "802.11n" },
  "time": 1790000000000
}
```

---

## 5. Transfers

| Method | Path | Scope | Description |
|---|---|---|---|
| POST | `/transfers` | transfer | Create/resume session |
| GET | `/transfers/{id}` | transfer | Status + verified chunk bitmap |
| PUT | `/transfers/{id}/chunks/{n}` | transfer | Upload chunk (octet-stream) |
| POST | `/transfers/{id}/complete` | transfer | Finalize + verify |
| DELETE | `/transfers/{id}` | transfer | Abort |
| GET | `/transfers` | transfer | List (filters: status, device, since) |

### `POST /transfers`
```json
{
  "name": "IMG_2041.MOV",
  "size": 2147483648,
  "mime": "video/quicktime",
  "kind": "send",                     // send | backup
  "rel_path": null,                   // optional folder under category
  "chunk_size": 4194304,
  "client_item_id": "content://media/external/video/media/1234",
  "root_hash": "blake3hex…",          // optional but required for backup kind
  "taken_at": 1789000000000,          // optional media time
  "target_device_id": null            // set for relay to another device
}
```
Response 201 (or 200 if resuming same `client_item_id`+`size`):
```json
{
  "transfer_id": "01J…",
  "chunk_size": 4194304,
  "chunk_count": 512,
  "have": { "encoding": "ranges", "ranges": [[0,339]] },
  "already_exists": false,
  "existing_file_id": null
}
```
If `root_hash` matches an existing file: `201` with `already_exists:true`, `existing_file_id` set, no upload needed.

Preflight failures: `507 INSUFFICIENT_STORAGE`.

### `PUT /transfers/{id}/chunks/{n}`
Headers: `Content-Type: application/octet-stream`, `Content-Length`, `X-Chunk-Hash: <blake3 hex of this chunk>`.
- `204` verified & persisted. Idempotent (re-PUT of verified chunk with same hash → 204).
- `409 HASH_MISMATCH` retry chunk.
- Parallel PUTs allowed (recommended 2–4).

### `POST /transfers/{id}/complete`
```json
{ "root_hash": "blake3hex…" }
```
200:
```json
{ "file_id": "01J…", "verified": true, "hash": "…", "size": 2147483648, "rel_path": "Videos/2026/10/IMG_2041.MOV" }
```
`422 ROOT_HASH_MISMATCH` → client should re-verify source and restart affected chunks (`GET /transfers/{id}` returns `invalid_chunks`).

### Events (SSE)
`GET /events` (text/event-stream, scope any) emits: `transfer.progress`, `transfer.completed`, `alert.created`, `device.revoked`, `storage.low`, `hub.shutting_down`. Clients may instead poll `/status`.

---

## 6. Files

| Method | Path | Scope | Description |
|---|---|---|---|
| GET | `/files?category=&path=&q=&cursor=` | files | List/browse/search |
| GET | `/files/{id}` | files | Metadata |
| GET | `/files/{id}/content` | files | Download (supports `Range`) |
| GET | `/files/{id}/manifest` | files | Chunk hash list |
| PATCH | `/files/{id}` | files | Rename/move (logical) |
| DELETE | `/files/{id}` | files | Move to trash |
| GET | `/trash` | files | List trash |
| POST | `/trash/{id}/restore` | files | Restore |
| DELETE | `/trash/{id}` | admin | Purge now |
| GET | `/library/summary` | files | Category counts/bytes, free space |

File object:
```json
{ "id":"01J…", "name":"IMG_2041.MOV", "category":"video", "mime":"video/quicktime",
  "size":2147483648, "hash":"…", "created_at":1790000000000, "modified_at":1789000000000,
  "path":"Videos/2026/10", "last_verified_at":1790100000000 }
```

---

## 7. Photos and backup

| Method | Path | Scope | Description |
|---|---|---|---|
| GET | `/photos/timeline?cursor=&limit=` | photos | Chronological items with thumb URLs |
| GET | `/photos/years` | photos | Year → month counts |
| GET | `/photos/{file_id}/thumb?size=256\|1024` | photos | Thumbnail (image/webp) |
| POST | `/backup/sources` | photos | Register/approve a source (camera roll) |
| PATCH | `/backup/sources/{id}` | photos | wifi_only, charging_only, enabled |
| POST | `/backup/sources/{id}/diff` | photos | Send `[{client_item_id, size, taken_at, hash?}]` → returns items needing upload |
| POST | `/backup/items/{id}/confirm-local-freed` | photos | Client reports local copy removed |
| GET | `/backup/sources/{id}/summary` | photos | Backed-up counts, pending, last run |
| GET | `/duplicates` | photos | Duplicate groups + reclaimable bytes |
| POST | `/duplicates/{group_id}/resolve` | photos | `{keep_file_id}` → others to trash |

`/backup/.../diff` response:
```json
{ "needed": ["item_a","item_c"], "already_backed_up": 480, "new_photos": 487, "new_videos": 63 }
```

**Free-phone-storage contract:** Client may offer deletion only for items where `backup_items.status = verified` AND client's local hash == Hub `hash`. Client re-fetches `GET /files/{id}` immediately before deletion.

---

## 7b. Storage health

| Method | Path | Scope | Description |
|---|---|---|---|
| GET | `/storage/health` | files | Disks, health, free space, last scrub, second-copy age |
| GET | `/alerts` | any | Active alerts |
| POST | `/alerts/{id}/ack` | any | Acknowledge |
| POST | `/storage/second-copy/run` | admin | Trigger second-copy (dashboard-preferred) |

---

## 8. Remote control (scope `remote`)

| Method | Path | Description |
|---|---|---|
| WS (`wss`) | `/remote/input` | Low-latency input channel |
| POST | `/remote/power` | `{action:"sleep"\|"restart"\|"shutdown"}` |
| POST | `/remote/media` | `{key:"play_pause"\|"next"\|"prev"\|"vol_up"\|"vol_down"\|"mute"}` |
| GET | `/remote/wake-info` | Hub MAC addresses + wake capability for client-side WoL |

### Input WebSocket messages (JSON, or CBOR later)
```json
{ "t":"mv", "dx": 12, "dy": -4 }
{ "t":"click", "b":"left" }          // left|right|middle, optional "n":2 for double
{ "t":"scroll", "dy": -3 }
{ "t":"key", "k":"Enter", "down":true }
{ "t":"text", "s":"hello" }
```
Rate-limited (≤ 500 msgs/s). Session auto-ends after 60 s idle. Tray shows "Remote active: <device>".

Power actions require confirmation flag `{"confirm":true}` and are written to `audit_log`.

---

## 9. Screen sharing (Phase 3)

Signaling over mTLS API; media over WebRTC on LAN.

| Method | Path | Description |
|---|---|---|
| POST | `/screen/view` | Start laptop→phone; body: `{offer_sdp, preset}` → `{answer_sdp, session_id}` |
| POST | `/screen/cast` | Start phone→laptop; same shape |
| POST | `/screen/{id}/ice` | Trickle ICE candidate |
| DELETE | `/screen/{id}` | Stop |
| GET | `/screen/capabilities` | Encoders, max preset (from hardware audit) |

---

## 10. Hardware and network

| Method | Path | Scope | Description |
|---|---|---|---|
| GET | `/hardware/audit` | any | Latest audit + ratings |
| POST | `/hardware/audit/run` | admin | Re-run |
| GET | `/network` | any | Interfaces, mode, link speed |
| POST | `/network/hotspot` | admin | Enable/disable hotspot (if supported) |

---

## 11. Limits and timeouts

| Item | Value |
|---|---|
| Chunk size | 1–8 MiB (default 4) |
| Max file size | 1 TiB (NTFS limit-bound) |
| Max concurrent chunk PUTs per transfer | 4 |
| Max concurrent transfers per device | 3 |
| Open transfer idle TTL | 7 days |
| Chunk PUT timeout | 30 s idle |
| Request body max (non-chunk) | 1 MiB |
| Rate limit (non-transfer) | 20 req/s/device |

---

## 12. Example flow: 2 GB video upload

```text
1  GET  /info                          → check api, features
2  GET  /ping?bytes=1048576            → estimate throughput
3  POST /transfers                     → transfer_id, have=[]
4  PUT  /transfers/{id}/chunks/0..511  (×3 parallel, X-Chunk-Hash each)
5  (Wi-Fi drops at chunk 340)          → client queues item state=queued
6  (Hub rediscovered via mDNS)         → POST /transfers (same client_item_id) → have=[[0,339]]
7  PUT chunks 340..511
8  POST /transfers/{id}/complete {root_hash} → 200 verified:true
9  Client marks queue_items.state=done
```
