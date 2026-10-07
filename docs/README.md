# Home Hub

> Turn a supported old laptop into your own private home cloud and wireless device hub. Phone, Mac and other devices share storage, files, backups and (later) screens over local Wi-Fi, with no cloud subscription and no internet dependency.

**Status:** Phase 0 (Windows prototype) · **Working title:** Home Hub · **Primary market:** India

---

## What it is

Home Hub is a lightweight background service that runs on an existing **Windows** laptop (no Linux, no repartitioning) and exposes it to the user's other devices as **"My Home"**.

```text
Phone / Mac / Tablet ──(local Wi-Fi, TLS 1.3, mTLS)──► Home Hub (old Windows laptop) ──► Your storage
```

Later (Phase 4) the same services ship as a dedicated lightweight Linux-based **Home Hub OS**. The OS is the delivery mechanism; the experience is the product.

## Core features

| Phase | Feature |
|---|---|
| 0 | Discovery (mDNS), QR pairing, chunked + resumable + verified transfer, queue-and-send-later, Android + macOS apps |
| 1 | Photo backup, gallery, duplicate detection, file browser, disk health, second-copy backup, phone trackpad/keyboard/power |
| 2 | Hardware audit, compatibility DB, hotspot fallback, wake support, scheduled backups, better iOS |
| 3 | Screen sharing (laptop→phone, phone→laptop) |
| 4 | Home Hub OS |
| 5 | Multi-hub storage, replication, wake relay, local AI |

## Principles

1. **Local-first.** Core features work with no internet. LAN-only by default.
2. **Never lose data.** Verify every byte (BLAKE3) before marking anything backed up.
3. **Powerful underneath, simple on top.** No IPs, ports, SMB paths or Linux jargon in the UI.
4. **Hardware-honest.** Supported-device list, hardware audit, graceful degradation, no "works on any laptop" promises.
5. **Old laptop resources belong to the user's data**, not to Home Hub. Idle target: <1% CPU, <80 MB RAM.

## Repository layout (target)

```text
home-hub/
├── README.md
├── docs/                      # this doc set
├── hub/                       # Rust workspace (Windows service now, Linux later)
│   ├── crates/
│   │   ├── hh-core/           # types, errors, config
│   │   ├── hh-net/            # mDNS, TLS, HTTP/2 server
│   │   ├── hh-auth/           # CA, pairing, device certs
│   │   ├── hh-transfer/       # chunking, hashing, sessions
│   │   ├── hh-storage/        # library, import, dedupe, SMART
│   │   ├── hh-photos/         # media metadata, gallery index
│   │   ├── hh-remote/         # input, power, media keys
│   │   ├── hh-db/             # SQLite migrations + queries
│   │   └── hh-service/        # Windows service entry + tray
│   └── dashboard/             # local web dashboard (static, embedded)
├── android/                   # Kotlin app
├── macos/                     # SwiftUI app
├── ios/                       # Phase 2
├── installer/                 # WiX / Inno Setup
└── tools/                     # bench, fake-client, test-data generator
```

## Document index

| File | Purpose |
|---|---|
| `README.md` | This overview |
| `PRD.md` | Product requirements, users, scope, success metrics |
| `TRD.md` | Technical architecture, stack, protocols, platform specifics |
| `IMPLEMENTATION_PLAN.md` | Phased work breakdown, milestones, task lists |
| `UI_UX_DESIGN.md` | Design principles, flows, screens, components, copy |
| `BACKEND_SCHEMA.md` | SQLite schema, on-disk layout, migrations |
| `API_SPEC.md` | HTTP/mDNS/pairing protocol reference |
| `SECURITY.md` | Threat model, crypto, key management, hardening |
| `TEST_PLAN.md` | Test strategy, performance benchmarks, hardware matrix |
| `AGENTS.md` | Instructions for coding agents working in this repo |

## Quick start (developer)

```bash
# Prereqs: Rust stable (MSVC toolchain on Windows), Node 20+, Android Studio, Xcode (macOS app)
git clone <repo> && cd home-hub/hub
cargo build --workspace
cargo run -p hh-service -- --console        # run as console app instead of Windows service
# dashboard: http://localhost:47801  (loopback only)
# pair a dev client:
cargo run -p hh-tools -- fake-client pair   # prints/consumes pairing QR payload
```

## Success criteria (MVP)

- Pair a phone with one QR scan in **<30 s**
- Transfer a **2 GB video** locally, no internet, no manual config
- Back up **~1,000 photos** after one-time approval
- Hub offline → transfers **queue and auto-complete** on return
- **Every** file integrity-verified
- Idle Hub: **very low CPU/RAM**

## License

TBD (recommend Apache-2.0 for core services; apps TBD).
