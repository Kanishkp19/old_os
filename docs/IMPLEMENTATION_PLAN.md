# Home Hub — Implementation Plan

**Principle:** Prove `Phone ↔ Hub ↔ Mac` on Windows first. Build the OS only after people want the experience.
Estimates assume 1–2 developers; adjust for team size. Durations are indicative.

---

## Windows release amendment (2026-10-08)

The approved Windows product plan supersedes the old milestone placement of Browser, YouTube, Music, Notes and Calculator: deliver these in the Windows release, with Tauri 2 / system WebView2 and shared Svelte views. Notes, playlists and browser data stay private to the Windows user. Browser/YouTube have explicit internet access and isolated, unprivileged webviews; core Hub remains offline-capable.

Follow [WINDOWS_PRODUCT_TODO.md](WINDOWS_PRODUCT_TODO.md) for the ten delivery phases and separate validation ledger. Preserve `/v1` compatibility, Rust service/session/tray boundaries, and Inno Setup as canonical installer. Complete implementation before comprehensive testing. Commit/push each phase to `implementation/windows-home-hub`. M6 stays untouched and requires a separate decision after technical validation and household pilot.

## 0. Milestone overview

| Milestone | Phase | Duration | Exit gate |
|---|---|---|---|
| M0 | Foundations | 1–2 wks | Repo, CI, skeleton service runs on Windows |
| M1 | Phase 0 prototype | 5–7 wks | 2 GB transfer + QR pair + queue + verify on 3 laptops |
| M2 | Phase 1 Home Cloud MVP | 8–10 wks | Pilot (20 households) metrics |
| M3 | Phase 2 Hardware & reliability | 5–6 wks | Compat DB, audit, wake, hotspot |
| M4 | Phase 3 Screen sharing | 4–6 wks | Stable 1080p30 on supported HW |
| M5 | Validation gate | 2–4 wks | Go/No-Go for OS |
| M6 | Phase 4 Home Hub OS | 10–14 wks | Bootable image + installer on supported list |
| M7 | Phase 5 Advanced | ongoing | Multi-hub, replication, local AI |

---

## 1. M0 — Foundations (Weeks 1–2)

### Tasks
- [ ] Create monorepo (layout in README); license; CODEOWNERS; contribution guide
- [ ] Rust workspace: `hh-core`, `hh-db`, `hh-net`, `hh-auth`, `hh-transfer`, `hh-service` (stubs)
- [ ] CI (GitHub Actions): `cargo fmt`, `clippy -D warnings`, `cargo test`, Windows + Linux build matrix
- [ ] Platform traits (Power, Input, DiskHealth, HwAudit, ServiceHost) with Windows impl stubs
- [ ] SQLite migration framework + first migration (`hub`, `settings`, `devices`, `pairing_tokens`, `transfers`, `transfer_chunks`, `files`)
- [ ] Logging (`tracing`), config loader, data dir (`%ProgramData%\HomeHub`)
- [ ] `hh-service --console` mode (runs as normal process for dev)
- [ ] Procure 3 reference old laptops (see TEST_PLAN hardware matrix)
- [ ] Figma tokens + core components (from UI_UX_DESIGN)

**Exit:** `cargo run -p hh-service -- --console` starts, creates DB, logs, serves `/v1/info` over plain TLS dev cert.

---

## 2. M1 — Phase 0: Windows prototype (Weeks 3–9)

### 2.1 Hub core
- [ ] **CA + certs** (`hh-auth`): generate Hub CA, server cert; DPAPI-protect CA key; device cert issuance from CSR; revocation list + custom rustls client verifier
- [ ] **Pairing:** token generation, QR payload, port 47802 pairing endpoint, lockout, audit log
- [ ] **mDNS** advertise `_homehub._tcp` + UDP broadcast fallback
- [ ] **HTTP/2 API** (axum + rustls): `/info`, `/ping`, `/status`, `/devices`
- [ ] **Transfer engine:** sessions, chunk PUT with BLAKE3 verify, preallocated `.part`, bitmap persistence, resume, complete (tree root verify), atomic rename, `files` insert
- [ ] **Library basics:** root selection, category routing (Videos/Photos/Documents/…), name collision handling
- [ ] **Download:** Range GET + manifest
- [ ] **Windows service host** + auto-start + recovery actions (restart on fail)
- [ ] **Installer** (WiX/Inno): service install, firewall rule (Private), data dir, uninstall options
- [ ] **Tray app** (`tray-icon`): status, pair QR window, open dashboard
- [ ] **Dashboard v0:** Overview (status, devices, transfers), Pair device

### 2.2 Android app (Kotlin)
- [ ] Project setup (Compose, Hilt, Room, OkHttp H2, WorkManager)
- [ ] mDNS discovery (NsdManager) + manual address fallback
- [ ] QR scan (CameraX + ML Kit), keystore keypair, CSR, pinned-CA pairing
- [ ] mTLS client (custom `SSLContext` with keystore key)
- [ ] Share-sheet target ("Send to Home")
- [ ] Chunked uploader with parallel PUTs, BLAKE3 (JNI/`blake3-jni` or Kotlin impl), resume
- [ ] Persistent queue (Room) + WorkManager triggers + `ConnectivityManager` callbacks
- [ ] Transfers screen (active/queued/history), notifications
- [ ] Connection-state header chip

### 2.3 macOS app (SwiftUI)
- [ ] Menu bar app, Bonjour browse, QR scan via Continuity camera/manual code
- [ ] Keychain identity, URLSession mTLS
- [ ] Drag-and-drop send, Finder Share extension
- [ ] Receive/download from Hub; phone→Mac via relay (basic)

### 2.4 Tooling
- [ ] `hh-tools fake-client`: scripted pair/upload/resume for CI
- [ ] `hh-tools bench`: throughput test with synthetic 2 GB file
- [ ] Fault injection: kill connection mid-chunk, kill Hub mid-transfer, power-cut simulation (VM hard reset)

### 2.5 Exit criteria (Phase 0)
| Check | Pass |
|---|---|
| Pair phone | <30 s median over 10 attempts |
| 2 GB transfer | Completes w/o internet, in line with throughput table |
| Resume | Disconnect at ~70% → resumes, no restart |
| Queue | Hub stopped → item queued → Hub started → auto-completes |
| Integrity | Corrupted chunk injected → detected, retried |
| Crash safety | Kill Hub during finalize → no partial file visible, no DB/file mismatch |
| Footprint | Idle <80 MB RAM, <1% CPU on i3-class |
| Reference laptops | All 3 pass |

---

## 3. M2 — Phase 1: Home Cloud MVP (Weeks 10–19)

### Workstream A: Photo backup and gallery
- [ ] Android: MediaStore scan, diff API, backup source approval, WorkManager periodic + charging/Wi-Fi constraints, foreground service for large runs
- [ ] Hub: `backup_sources`, `backup_items`, diff endpoint, media metadata extraction (EXIF), thumbnail pipeline (idle-priority worker)
- [ ] Gallery UI (phone + dashboard): timeline, Year→Month, viewer
- [ ] "Free phone storage" flow (verify → re-check → system delete dialog)
- [ ] Backup of ~1,000 photos test + performance tuning (batch diff, concurrent small-file uploads)

### Workstream B: Storage management
- [ ] File browser (categories, search, sort), trash with restore
- [ ] Existing-data scanner + Keep/Import/Later wizard
- [ ] Storage usage UI
- [ ] Exact-duplicate detection + reclaim flow (to trash only)

### Workstream C: Disk health and safety
- [ ] SMART/WMI collector, snapshots, health states
- [ ] Free-space thresholds + alerts + push (SSE + FCM optional; LAN-only default uses SSE/polling)
- [ ] Integrity scrubber (rate-limited), `integrity_events`
- [ ] Second-copy to external drive (incremental, verified) + "copies" indicator in UI
- [ ] Persistent single-copy / failing-drive banners

### Workstream D: Remote
- [ ] `hh-session.exe` user-session helper + named-pipe IPC
- [ ] Input WebSocket, trackpad/keyboard UI, media keys, power controls, tray "remote active" indicator
- [ ] Scope `remote` gating + audit log

### Workstream E: Hardening and pilot
- [ ] Rate limiting, request size limits, audit log UI
- [ ] Code signing for installer and binaries; SmartScreen reputation plan
- [ ] Crash reporting (local, opt-in export)
- [ ] Pilot program: 20 households, weekly check-ins, in-app feedback form
- [ ] Usability tests (5 non-technical users)

**Exit (Phase 1):** Pilot targets in PRD §9; no open P0 integrity/security bugs.

---

## 4. M3 — Phase 2: Hardware and reliability (Weeks 20–25)

- [ ] Hardware audit collector + rating engine + dashboard Hardware page
- [ ] Compatibility database (YAML in repo, signed bundle) + "Supported / Untested / Unsupported" badge
- [ ] Network manager: prefer Ethernet, detect Wi-Fi standard, link-speed hints
- [ ] Hotspot mode (Windows Mobile Hotspot API / `netsh wlan hostednetwork` where supported)
- [ ] Wake: WoL registry, magic packet sender in Android/macOS, capability detection, docs for BIOS/NIC setup; RTC scheduled wake (Task Scheduler wake timers)
- [ ] Scheduled backups
- [ ] iOS app MVP: pairing, send, foreground backup, BGProcessingTask-based opportunistic backup; set clear UX expectations
- [ ] Perceptual duplicate detection (images)
- [ ] Hindi localization
- [ ] Optional update channel (signed manifests, rollback)

**Exit:** Audit accurate on ≥8 laptop models; wake success matrix documented; iOS backup works in foreground + best-effort background.

---

## 5. M4 — Phase 3: Screen sharing (Weeks 26–31)

- [ ] Capture in helper (WGC/DXGI), encoder selection (MF HW → x264 fallback)
- [ ] WebRTC (`webrtc-rs`) LAN-only, signaling over mTLS API
- [ ] Android viewer; macOS viewer
- [ ] Phone→Laptop cast receiver window
- [ ] Quality presets and auto-select via audit; latency/CPU benchmarks
- [ ] Input passthrough from viewer (optional) reusing remote channel

**Exit:** 1080p30 stable on HW-encode laptops; graceful "Limited" experience on weak laptops; glass-to-glass <150 ms on LAN (target).

---

## 6. M5 — Validation gate (Weeks 32–35)

Decision metrics (from PRD §9): ≥20 pilot households active 4 weeks, ≥60% unaided photo backup, NPS ≥40, unprompted "my home computer" language.
**Go** → M6. **No-go** → iterate UX/hardware focus or pivot features (e.g., consumer-friendly photo vault on Windows only).

---

## 7. M6 — Phase 4: Home Hub OS (Weeks 36–50)

- [ ] Port OS-abstraction traits to Linux (Power via logind, Input via uinput, DiskHealth via smartctl/udev, HwAudit via sysfs/dmidecode)
- [ ] Base image: minimal Linux, Wayland kiosk compositor, systemd units for `hh-service`
- [ ] Shell UI (WebView/Tauri or native): Photos, Files, Browser, YouTube PWA, Music, Notes, Calculator, Devices, Settings
- [ ] Installer: USB creator (Windows/macOS), hardware audit, keep/migrate/erase flow, NTFS data import
- [ ] A/B updates, rollback, signed images
- [ ] Hardware enablement: Wi-Fi/firmware blobs, GPU, audio, touchpad; supported list testing
- [ ] Power User Mode

**Exit:** Install on ≥10 supported models from USB in <15 min by a non-technical tester; update/rollback test passes.

---

## 8. M7 — Phase 5 (future)
Multi-hub storage pool, replication, wake relay device, local AI (indexing + small models), additional device classes (TV, printer, camera).

---

## 9. Team and roles

| Role | Responsibility |
|---|---|
| Systems/Rust dev | Hub core, transfer, storage, Windows integration |
| Android dev | App, backup, queue |
| Apple dev | macOS (then iOS) |
| Designer/Frontend | Figma system, dashboard, copy, usability research |
| QA/Hardware | Laptop matrix, fault injection, perf |

Solo-dev path: Rust Hub + Android first (weeks 1–14), macOS after, dashboard minimal, defer iOS.

## 10. Critical path and dependencies

```text
CA/pairing → mTLS API → transfer engine → Android uploader → queue
                                    └→ installer/service → pilot
transfer engine → photo backup → gallery → free-space flow
SMART/health → second copy → alerts   (must land before inviting pilot users to rely on it)
user-session helper → remote control → screen sharing
```

## 11. Risk register (implementation)

| Risk | Likelihood | Mitigation |
|---|---|---|
| BLAKE3 on Android/Swift performance | Med | Use native libs (JNI / Swift package), benchmark early |
| Windows Session 0 isolation surprises | High | Design helper process from the start (M2-D); spike in M0 |
| Wi-Fi old adapters unstable | High | Reference laptops incl. 802.11n; adaptive chunk/parallelism |
| Code-signing/SmartScreen delays | Med | Start procurement in M0 |
| Android background restrictions (OEM killers) | High | Foreground service for long runs, user guidance, test on Xiaomi/Realme/Samsung |
| Scope creep into OS | High | Enforce gate M5 |
| SMART access varies | Med | WMI + smartctl fallback; show "unknown" honestly |

## 12. Definition of done (any feature)
- Unit + integration tests; fault-injection where it touches data
- Security review checklist (SECURITY.md §10)
- Docs updated (API_SPEC / schema migration)
- Works on all reference laptops
- Accessibility + microcopy reviewed
- No new idle CPU/RAM regression beyond budget
