# Home Hub

Turn an old laptop into your family's private cloud. Your photos, files, and
backups live on hardware you own, reachable only on your home Wi-Fi — no
accounts, no subscriptions, no uploads to anyone else's computer.

```
┌─────────────┐   QR + one-time token   ┌──────────────────────────┐
│  Android /  │ ──────────────────────▶ │  Old laptop (Windows)    │
│  macOS /    │                         │  hh-service (Rust)       │
│  iOS        │  TLS 1.3 mutual auth    │  ├─ API        :47800    │
│  clients    │ ◀────────────────────── │  ├─ Dashboard  :47801    │
└─────────────┘  chunked BLAKE3-verified│  │  (loopback only)       │
                 resumable transfers    │  ├─ Pairing    :47802    │
                                        │  └─ mDNS       :5353     │
                                        │  SQLite · BLAKE3 · mDNS  │
                                        └──────────────────────────┘
```

## What's in this repo

| Path | What it is |
|---|---|
| `hub/` | Rust workspace — the service that runs on the laptop (12 crates) |
| `hub/dashboard/` | The loopback web dashboard (served on 127.0.0.1:47801) |
| `android/` | Android app (Kotlin, Compose, WorkManager, share-sheet target) |
| `macos/` | macOS menu-bar app (SwiftUI, Bonjour, drag-drop) |
| `ios/` | iOS MVP skeleton (Phase 2) |
| `compat/` | Hardware compatibility database (YAML) |
| `installer/` | Inno Setup + WiX installer scripts for the hub |
| `tools/` | Fault-injection harness + synthetic test-data generator |
| `docs/` | The specification set (PRD, TRD, API, schema, security, tests…) |
| `docs/m5/` | Pilot validation-gate package |

## Quick start

```bash
# 1. Build the hub service (Rust stable)
cd hub && cargo build --release

# 2. Run it in console mode on the laptop
./target/release/hh-service --console

# 3. Open the dashboard
start http://127.0.0.1:47801     # shows a QR code

# 4. Build + install the Android app
cd ../android && ./gradlew installDebug

# 5. Scan the QR with the phone → paired. Share anything to "Home Hub".
```

Full build/run/test instructions, including how to exercise the transfer
protocol without a phone, are in **[IMPLEMENTATION_GUIDE.md](IMPLEMENTATION_GUIDE.md)**.

## Design invariants (read before touching anything)

1. **Never lose or corrupt user data.** A file is "stored" only after
   whole-file hash verification → fsync → atomic rename → DB commit, in that
   order. Anything less is a bug.
2. **LAN-only.** No UPnP, no public listeners, no cloud relay. The dashboard
   binds loopback only; everything else requires mutual TLS.
3. **Pairing is physical consent.** QR + one-time token (or a 6-digit code
   confirmed on the Hub), CA fingerprint verified before the token is sent.
4. **Never delete user data without an explicit, verified gate.** Trash has a
   retention window; "free phone storage" requires Hub-side verification first.
5. **Plain words.** UI copy says "Sending", not "uploading chunk manifest".

## Status

Source-complete through milestone **M5** (specs in `docs/IMPLEMENTATION_PLAN.md`).
The Android app is the reference client; macOS is functional with a staged
pairing CSR; iOS is a bring-up skeleton. See the implementation guide for the
exact verification status of each component.
