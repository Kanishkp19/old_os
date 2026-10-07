# Home Hub — Product Requirements Document (PRD)

**Version:** 0.1 · **Owner:** Kanishk · **Status:** Draft for Phase 0/1

---

## 1. Summary

Home Hub turns an unused, supported old laptop into a private always-on **home cloud and device hub**. Phones, Macs and tablets on the same local network can send files, back up photos, browse storage and control the laptop, with no recurring cloud fees and no internet needed.

**Positioning:** One product, one setup, one identity, one hub. Not a NAS for IT admins, not a Linux distro, not a single-purpose transfer app.

## 2. Problem statement

| # | Problem | Today's workaround | Pain |
|---|---|---|---|
| P1 | Phone storage full of photos/videos | Paid cloud (Google/iCloud) | Recurring cost, esp. price-sensitive users |
| P2 | Old laptops sit unused | Drawer / resale | Wasted storage, RAM, Wi-Fi, USB |
| P3 | Moving files between own devices | Cable, WhatsApp, Telegram, USB, cloud | No "select → send to my device" simplicity |
| P4 | Personal data lives on third-party servers | Accept it | Privacy concern |
| P5 | Home depends on internet | Nothing | Outage = no access to own files |

## 3. Target users

| Persona | Description | Key need |
|---|---|---|
| **Student (primary)** | Old family laptop + smartphone + Mac/Windows laptop, many photos/videos, won't pay for cloud | Free, local backup + fast transfer |
| **Family (primary)** | Multiple phones/laptops/tablets, 1+ idle laptop | Shared household storage |
| **Non-technical user** | Doesn't know IPs, SMB, Linux | "This is my Home Computer" mental model |

**Initial market:** India (high old-laptop density, cloud subscriptions meaningful).

## 4. Goals and non-goals

### Goals
- G1: One-scan pairing; zero manual networking.
- G2: AirDrop-simple local transfer that is resumable and verified.
- G3: Trustworthy photo backup the user can rely on to free phone space.
- G4: Work fully offline on LAN.
- G5: Be honest about hardware capability and disk health.
- G6: Run as a tiny background footprint on weak hardware.

### Non-goals (v1)
- Replacing Windows (Phase 4 only, after validation)
- Public-internet exposure / remote access outside home (default off; future opt-in)
- Distributed storage / multi-hub (Phase 5)
- Local AI search (Phase 5)
- Full remote desktop replacement (screen sharing is Phase 3)
- Guaranteed wake-from-off on all hardware

## 5. Scope by phase

| Phase | Name | Outcome |
|---|---|---|
| 0 | Windows prototype | Phone ↔ Hub ↔ Mac transfers proven |
| 1 | Home Cloud MVP | Photo backup, gallery, files, dedupe, disk health, remote controls |
| 2 | Hardware & reliability | Audit, compat DB, hotspot, wake, scheduling, iOS improvements |
| 3 | Screen sharing | View on Phone / Cast to Laptop |
| 4 | Home Hub OS | Bootable appliance OS |
| 5 | Advanced | Multi-hub, replication, wake relay, local AI |

## 6. Functional requirements

Priority: **P0** must-have for phase · **P1** should · **P2** could.

### 6.1 Install and service
| ID | Requirement | Pri |
|---|---|---|
| FR-1.1 | Single installer (MSI/EXE) installs Hub as Windows service, auto-starts on boot | P0 |
| FR-1.2 | Installer adds firewall rule for LAN only (Private profile) | P0 |
| FR-1.3 | System tray app shows status: online, free storage, connected devices, active transfers | P0 |
| FR-1.4 | Does not modify, delete or rearrange existing Windows data | P0 |
| FR-1.5 | Clean uninstall, with option to keep or remove Hub library | P0 |

### 6.2 Discovery and pairing
| ID | Requirement | Pri |
|---|---|---|
| FR-2.1 | Hub advertises `_homehub._tcp` via mDNS; fallback UDP broadcast | P0 |
| FR-2.2 | Client lists nearby Hubs by friendly name ("Kanishk's Home Hub") | P0 |
| FR-2.3 | Pairing by one QR scan; one-time token, 5-minute expiry | P0 |
| FR-2.4 | No account creation required | P0 |
| FR-2.5 | Each device gets own identity; "Remove Device" revokes immediately | P0 |
| FR-2.6 | Manual pairing code (6 digits) fallback if camera unavailable | P1 |

### 6.3 Transfer
| ID | Requirement | Pri |
|---|---|---|
| FR-3.1 | Send files/folders from client to Hub; Hub to client | P0 |
| FR-3.2 | Chunked (4 MiB), per-chunk hash, whole-file hash verification | P0 |
| FR-3.3 | Resume from last verified chunk after disconnect | P0 |
| FR-3.4 | Queue-and-send-later when Hub unreachable; auto-start on return | P0 |
| FR-3.5 | Show estimated time based on measured throughput | P1 |
| FR-3.6 | Device-to-device (phone→Mac) via Hub relay | P1 |
| FR-3.7 | Transfer history with status | P1 |

### 6.4 Storage
| ID | Requirement | Pri |
|---|---|---|
| FR-4.1 | Present clean library (Photos, Videos, Documents, Music, Backups, Downloads), never raw Windows paths | P0 (P1 phase) |
| FR-4.2 | Detect existing files by category with sizes; choose Keep / Import / Later | P1 |
| FR-4.3 | Storage usage dashboard | P1 |
| FR-4.4 | Second-copy backup to external HDD/SSD | P1 |
| FR-4.5 | Optional encryption at rest (BitLocker detect/recommend; app-level later) | P2 |

### 6.5 Photo backup and gallery
| ID | Requirement | Pri |
|---|---|---|
| FR-5.1 | Detect new photos/videos on phone; "Back up now?" one-time approval | P0 |
| FR-5.2 | Backup is verified before an item is marked backed-up | P0 |
| FR-5.3 | "Free phone storage" offered only after verification | P0 |
| FR-5.4 | Gallery grouped Year → Month; thumbnails | P1 |
| FR-5.5 | Exact duplicate detection with reclaimable-space estimate | P1 |
| FR-5.6 | Perceptual duplicate detection | P2 |
| FR-5.7 | Events / places / people / semantic search | P2 (Phase 5) |

### 6.6 Storage health
| ID | Requirement | Pri |
|---|---|---|
| FR-6.1 | Monitor SMART (where available), free space, disk errors | P0 (P1 phase) |
| FR-6.2 | Scheduled integrity scrub of stored files against hashes | P1 |
| FR-6.3 | Warn on failing drive; prompt immediate second copy | P0 (P1 phase) |
| FR-6.4 | UI never implies a single drive equals a backup | P0 |

### 6.7 Remote control
| ID | Requirement | Pri |
|---|---|---|
| FR-7.1 | Phone as trackpad + keyboard (authenticated, LAN) | P1 |
| FR-7.2 | Media keys (play/pause/volume) | P1 |
| FR-7.3 | Power: Sleep / Restart / Shutdown | P1 |
| FR-7.4 | Wake: Wake-on-LAN where supported; else queue | P2 (Phase 2) |

### 6.8 Network modes
| ID | Requirement | Pri |
|---|---|---|
| FR-8.1 | Home Wi-Fi mode (default) | P0 |
| FR-8.2 | Ethernet preferred when present | P0 |
| FR-8.3 | Hotspot mode where hardware supports | P2 (Phase 2) |
| FR-8.4 | Distinguish "no internet" from "no local network" in UI | P0 |

### 6.9 Hardware audit
| ID | Requirement | Pri |
|---|---|---|
| FR-9.1 | Audit CPU, RAM, storage, Wi-Fi standard, Ethernet, battery, camera | P1 (Phase 2) |
| FR-9.2 | Rate each capability and adapt features (e.g., disable streaming on weak hardware) | P1 |
| FR-9.3 | Supported-device list; unsupported hardware allowed with warning | P1 |

### 6.10 Screen sharing (Phase 3)
- FR-10.1 Laptop → Phone ("View on Phone"), local WebRTC
- FR-10.2 Phone → Laptop ("Cast to Laptop")
- FR-10.3 Quality presets; HW encode selection

## 7. Non-functional requirements

| Category | Requirement |
|---|---|
| Performance (idle) | CPU <1% avg, RAM <80 MB resident (service) |
| Performance (transfer) | ≥70% of measured link throughput; targets below |
| Reliability | No data loss on crash/power loss mid-transfer; atomic finalize |
| Security | LAN-only default; TLS 1.3 + mTLS; revocable device identity |
| Privacy | No telemetry without explicit opt-in; no cloud dependency |
| Compatibility | Windows 10 22H2 / 11, x64, ≥4 GB RAM; Android 9+; macOS 13+ |
| Usability | Non-technical user completes setup unaided; no jargon |
| Accessibility | WCAG AA contrast, screen-reader labels, scalable text |
| Localization | English v1; Hindi next; string externalization from day one |

### Indicative throughput targets

| Connection | Throughput | ~2 GB |
|---|---:|---:|
| Gigabit Ethernet | 100–115 MB/s | 20–30 s |
| Wi-Fi 6, 5 GHz | 40–80 MB/s | 30–60 s |
| Wi-Fi 5, 5 GHz | 25–50 MB/s | 1–1.5 min |
| 802.11n 2.4 GHz | 5–15 MB/s | 2.5–7 min |

Targets, not guarantees. UI shows measured estimate.

## 8. Key user journeys

1. **First-time setup:** Install on laptop → tray shows QR → install phone app → scan → "Connected to My Home".
2. **Send a video:** Share sheet → My Home → progress → verified ✓.
3. **Photo backup:** Open app → "487 photos, 63 videos new. Back up now?" → approve → verified → optional "Free phone storage".
4. **Hub offline:** Send file → "Home Hub unavailable. Queued." → Hub returns → auto-transfer + notification.
5. **Disk warning:** Tray + phone notification "Storage drive may be failing. Back up now."
6. **Remote control:** Open Remote → trackpad/keyboard/power.

## 9. Success metrics

### MVP (Phase 0–1)
| Metric | Target |
|---|---|
| Time to pair | <30 s median |
| 2 GB local transfer | Completes without manual config; within throughput table |
| Photo backup | ~1,000 photos after single approval |
| Queue recovery | 100% of queued transfers complete after Hub returns |
| Integrity | 100% of finalized files hash-verified |
| Idle footprint | CPU <1%, RAM <80 MB |
| Crash-safety | 0 data-loss incidents in power-cut test suite |

### Product validation (decision gate for Phase 4)
- ≥ N pilot households (suggest 20) active weekly for 4 weeks
- ≥60% of pilots complete photo backup unaided
- NPS ≥ 40 among pilots
- Qualitative: users call it "my home computer", not "a server"

## 10. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Hardware diversity | Flaky behavior | Supported list, audit, graceful degradation |
| Old drives fail | Data loss, trust loss | SMART, scrub, second-copy prompts |
| iOS background limits | Weak auto-backup | Design for BGProcessingTask; set expectations; Android first |
| Wake unreliable | Missed transfers | Queue is the guarantee |
| Security breach | Catastrophic | LAN-only, mTLS, minimal surface, signed updates |
| Competition (LocalSend, Immich, Nextcloud, Synology, KDE Connect) | Differentiation | Integration + simplicity, not individual features |
| Windows Defender/SmartScreen friction | Install drop-off | Code signing, reputation building |

## 11. Competitive landscape

| Product | Covers | Gap Home Hub fills |
|---|---|---|
| LocalSend | File transfer | No storage/backup/remote |
| Immich | Photos | Requires Docker/server skills |
| Nextcloud | Files/cloud | Admin-heavy |
| Synology/QNAP | NAS | Extra hardware cost |
| KDE Connect | Device integration | No storage/photos; weak on Windows/iOS |
| Sunshine/Moonlight | Streaming | Single purpose |
| ChromeOS Flex | Revive old PC | Not a hub |

## 12. Open questions

1. Hub implementation language final: Rust (recommended) vs Go.
2. Pricing: free + paid tier (multi-hub/AI) vs one-time license vs fully free.
3. Hindi + regional language timeline.
4. Pilot recruitment channel (college communities, family networks).
5. Code-signing certificate procurement for India-based entity.

## 13. Release criteria

- **Phase 0 exit:** All 6 MVP success criteria met on ≥3 reference laptops.
- **Phase 1 exit:** Pilot cohort metrics hit; no P0 data-integrity bugs open.
