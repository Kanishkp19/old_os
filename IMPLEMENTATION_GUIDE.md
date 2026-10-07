# Home Hub — Implementation Guide

How to build, run, test, and ship every piece of this repo. Read the
"Verification status" section first so you know exactly where things stand.

---

## 0. Verification status (honest)

This tree was written against the specs in `docs/` but **has not been compiled
in CI yet**. Before your first demo, run the checklist in §7. The spots where
the upstream crate APIs move fastest — and where you should expect small
adjustments — are flagged in code comments:

| Spot | Why it may need a touch |
|---|---|
| `hh-auth/src/ca.rs` — CSR signing | `rcgen 0.13` changed CSR API twice recently |
| `hh-net/src/tls.rs` — `ClientCertVerifier` | rustls 0.23 verifier trait shape |
| `hh-tools/src/fake_client.rs` — `Identity::from_pem` | reqwest feature-gated (`rustls-tls`) |
| `hh-net/src/mdns.rs` | mdns-sd 0.11 TXT record API |
| macOS `Pairing.swift` — CSR builder | intentionally stubbed; port from Android |
| iOS `Pairing.swift` | intentionally stubbed (M3 skeleton) |

Everything else (transfer engine, DB layer, storage, photos, remote, hw,
stream, Android client) uses stable, mainstream APIs.

---

## 1. Prerequisites

| Machine | Needs |
|---|---|
| Build machine (any OS) | Rust stable (rustup), `cargo`, Git |
| Hub laptop (Windows 10+) | Nothing extra — the service is self-contained |
| Android dev | Android Studio, JDK 17, Android SDK 35 |
| macOS/iOS dev | Xcode 16+ on a Mac |

Optional but useful: `b3sum` (fault-injection script), `sqlite3` (metrics.sql),
Inno Setup 6 or WiX 4 (installer builds).

---

## 2. Build and run the hub

```bash
cd hub
cargo build --release
```

Run in console mode (first run creates the CA, DB, and config under the data dir):

```bash
# Windows
target\release\hh-service.exe --console --data-dir C:\hh-data

# Linux/macOS (for development)
./target/release/hh-service --console --data-dir ./hh-data
```

On startup the service:

1. opens `hub.db` and runs forward-only migrations (with pre-migration backups);
2. loads or creates the Hub CA (key is DPAPI-protected on Windows);
3. reconciles interrupted transfers (orphan `.part` files are GC'd);
4. starts three listeners:
   - **47800** mTLS API (LAN clients only, source-IP allowlist),
   - **47801** dashboard on **127.0.0.1 only**,
   - **47802** pairing (TLS, token-gated, logically time-boxed);
5. advertises `_homehub._tcp.local.` via mDNS and answers the UDP fallback;
6. spawns background workers: thumbnails (idle-priority), scrub (50 files/h),
   SMART poll (6h), trash/GC (hourly).

Open `http://127.0.0.1:47801/` → **Add a device** → QR + 6-digit code appear.

### Install as a Windows service (production)

```powershell
# As Administrator, from the install folder:
sc.exe create HomeHub binPath= "C:\Program Files\HomeHub\hh-service.exe" start= auto
sc.exe start HomeHub
```

Or build the installer (§6), which does this plus firewall rules for you.

---

## 3. Exercise the protocol without a phone (recommended first!)

`hh-tools` includes a fake client that performs the exact client-side flow:

```bash
cd hub

# Terminal 1: the hub
cargo run --release -p hh-service -- --console --data-dir /tmp/hh-data

# Terminal 2: discover, pair (open the dashboard QR page first), upload
cargo run --release -p hh-tools -- discover
cargo run --release -p hh-tools -- pair --qr "homehub://pair?h=...&t=...&fp=...&a=...&n=..." \
    --out /tmp/identity
cargo run --release -p hh-tools -- upload --identity /tmp/identity --file ./some-big-file.bin

# Benchmark throughput on your LAN
cargo run --release -p hh-tools -- bench --identity /tmp/identity --size 2GiB
```

The upload command demonstrates the whole reliability story: create transfer
with a stable `client_item_id`, PUT 4 MiB chunks with `X-Chunk-Hash`, resume
from the server's `have` ranges after a simulated crash, then `complete` with
the BLAKE3 root.

---

## 4. Build the Android app

```bash
cd android
./gradlew assembleDebug      # or installDebug with a device attached
```

Flow to verify end-to-end:

1. Fresh install → Home tab → **Connect** → scan the dashboard QR.
2. Share a photo from Google Photos → **Home Hub** appears → item lands in
   the queue ("Waiting").
3. On home Wi-Fi it sends automatically; turn on airplane mode mid-transfer,
   then off → it resumes without re-sending verified chunks.
4. **Remote control** tab → move the laptop cursor from the phone.
5. **Devices** tab → revoke a device → its next request is refused.

Hindi UI: set the emulator/device locale to हिन्दी — all strings ship in
`values-hi`.

---

## 5. macOS and iOS

- **macOS**: open `macos/` in Xcode, create an App target (see
  `macos/README.md`). Menu-bar app with Bonjour discovery and drag-drop queue.
  The PKCS#10 CSR builder is the one stub — port it from
  `android/.../PairingClient.kt` (it is ~40 lines of DER) or adopt
  swift-certificates.
- **iOS**: M3 skeleton under `ios/` — QR scanner, App-Group queue, share
  extension are real; pairing throws "not implemented" on purpose so nobody
  mistakes it for a shipping client.

---

## 6. Build the installer

```powershell
# Option A: Inno Setup 6
iscc installer\homehub.iss

# Option B: WiX 4
wix build installer\wix\Product.wxs -o HomeHubSetup.msi
```

Both installers: register the service (auto-start, restart-on-failure), add
**Private-profile** firewall rules for 47800/47802/5353 only, create
`%ProgramData%\HomeHub`, and on uninstall **ask before deleting data**
(default: keep). The dashboard port needs no firewall rule (loopback).

---

## 7. First-run checklist (do this before the pilot)

```bash
cd hub
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check                 # license/ban policy
```

Then the protocol-level gauntlet (hub running, identity from `hh-tools pair`):

```bash
tools/fault_injection.sh 192.168.1.50:47800 /tmp/identity
```

It asserts: mid-transfer kill → resume works; corrupted chunk → rejected;
wrong root hash → 422; no client cert → refused. All four must pass.

Generate a synthetic library for gallery/perf testing:

```bash
python3 tools/gen_test_data.py --out /tmp/hhdata --photos 500 --dupes 20
```

CI (`.github/workflows/ci.yml`) runs fmt/clippy/tests on Linux + Windows,
an `audit-check`, cargo-deny, Android unit tests, and a nightly job wired for
the fault-injection harness.

---

## 8. Run the M5 pilot

Everything you need is in `docs/m5/`: the recruiting/onboarding/interview
playbook, the read-only `metrics.sql` to collect from each hub, the NPS
script, and the go/no-go decision template. The gate: ≥ 20 households ×
4 weeks, ≥ 60% unaided backup, zero data loss, NPS ≥ 40.

---

## 9. Repo conventions

- Conventional commits (`feat(hub-transfer): …`); PR checklist in
  `CONTRIBUTING.md`; `@kanishk` owns everything (`CODEOWNERS`).
- All OS-specific code behind the traits in `hh-core/src/platform.rs`
  (this is what makes the Phase-4 Linux port cheap).
- Migrations are forward-only and numbered; never edit an applied one.
- No secrets in logs — tokens, keys, and file contents are never logged
  (`SECURITY.md` §8).
