# Home Hub — Test Plan

**Goal:** Prove the MVP promise: pair fast, transfer reliably, never lose data, stay light on old hardware.

---

## 1. Test pyramid

| Level | Tooling | Scope |
|---|---|---|
| Unit | `cargo test`, JUnit, XCTest | Chunking, hashing, bitmap, path sanitizer, cert validation, state machines |
| Integration | `hh-tools fake-client` + test Hub in temp dir | Pair, upload/resume/complete, queue, backup diff, revoke |
| Fault injection | Custom harness, VM snapshots | Network drops, process kill, power cut, disk full |
| E2E device | Real phones + reference laptops | Real Wi-Fi, real files |
| Performance | `hh-tools bench`, Windows perf counters | Throughput, CPU, RAM |
| Security | Fuzzers, pentest scripts | See SECURITY.md §12 |
| Usability | 5–10 non-technical users | Setup, pairing, send, backup |

## 2. Reference hardware matrix

| ID | Class | Example spec | Network | Disk | Purpose |
|---|---|---|---|---|---|
| L1 | Low-end | Dual-core i3 (gen 2–4), 4 GB, Win 10 | 802.11n 2.4/5 GHz, 100 Mb Ethernet | 500 GB 5400 rpm HDD | Worst-case floor |
| L2 | Mid | i5 (gen 5–7), 8 GB, Win 10/11 | 802.11ac, Gigabit | 1 TB HDD or 256 GB SSD | Typical target |
| L3 | Upper | i5/i7 (gen 8+), 8–16 GB, Win 11 | Wi-Fi 6, Gigabit | SSD | Streaming / best-case |
| L4 | Edge | Laptop with failing/marginal HDD (SMART caution) | any | Used drive | Health alerts |
| P1–P3 | Phones | Android 9, 12, 14 (incl. Xiaomi/Realme/Samsung) | Wi-Fi 5/6 | — | OEM battery killers |
| M1 | Mac | macOS 13/14 | Wi-Fi 6 | — | macOS app |
| R1 | Routers | Budget ISP router, mesh router, AP-isolation enabled | — | — | Discovery edge cases |

## 3. Functional test suites

### 3.1 Pairing and identity
| ID | Case | Expected |
|---|---|---|
| PA-01 | Scan valid QR | Paired <30 s |
| PA-02 | Expired token | 410, clear UI error |
| PA-03 | Reused token | Rejected |
| PA-04 | 6 wrong token attempts | Locked (423) |
| PA-05 | Wrong CA fingerprint (evil twin) | Client aborts before sending token |
| PA-06 | Manual code | Requires Hub confirmation |
| PA-07 | Remove device while connected | Active connections dropped; handshake fails after |
| PA-08 | Cert renewal at <30 days | Seamless |
| PA-09 | Hub reinstall without CA backup | Clients detect fingerprint change, require re-pair |

### 3.2 Discovery and network
| ID | Case | Expected |
|---|---|---|
| NW-01 | mDNS on normal router | Hub listed |
| NW-02 | mDNS blocked | UDP broadcast fallback finds Hub |
| NW-03 | AP/client isolation on | Clear message "Your router blocks device-to-device traffic" |
| NW-04 | Hub IP changes (DHCP) | Client reconnects without user action |
| NW-05 | Internet unplugged, LAN up | All core features work; UI says "no internet, still works" |
| NW-06 | Phone on mobile data/other network | "Not on home Wi-Fi"; queue |
| NW-07 | Ethernet + Wi-Fi both active | Prefers Ethernet, advertises correct addresses |

### 3.3 Transfer
| ID | Case | Expected |
|---|---|---|
| TR-01 | 1 KB, 10 MB, 2 GB, 20 GB files | All verified |
| TR-02 | 0-byte file | Handled |
| TR-03 | Wi-Fi off at 70% | Paused → resumes from verified chunk |
| TR-04 | Hub service killed mid-transfer | After restart, resume works; no corrupt file visible |
| TR-05 | Client app killed mid-transfer | Queue resumes on relaunch/WorkManager |
| TR-06 | Flip bits in a chunk in flight | 409, chunk retried, final verified |
| TR-07 | Flip bits in `.part` file on disk | `complete` returns 422; affected chunks re-requested |
| TR-08 | Disk full during upload | 507/503, clean abort, no partial file in library, clear UI |
| TR-09 | Same file sent twice | Dedupe via hash: no second upload |
| TR-10 | Source file modified during upload | Detected (mtime/size), re-hash/restart |
| TR-11 | Filename edge cases (unicode, emoji, long path, `CON`, trailing dot, `:`) | Safely sanitized; no traversal |
| TR-12 | 3 concurrent transfers | Fair progress; caps respected |
| TR-13 | Phone→Mac relay | Delivered + verified |
| TR-14 | Power cut during finalize | On boot: either complete file + DB row, or neither (no orphan/partial) |

### 3.4 Queue-and-send-later
| ID | Case | Expected |
|---|---|---|
| QU-01 | Hub off → send → Hub on | Auto-transfers, notification |
| QU-02 | 100 queued items, Hub returns | Processed in order, backoff sane |
| QU-03 | Phone reboot with queue | Queue persists |
| QU-04 | Source deleted before send | Marked failed_perm with clear message |
| QU-05 | Airplane mode toggle | No battery-draining retry loops (measure) |

### 3.5 Photo backup
| ID | Case | Expected |
|---|---|---|
| PB-01 | 1,000 photos, one approval | All backed up, verified |
| PB-02 | Mixed HEIC/JPEG/MP4/Live Photo | Preserved; thumbs or placeholder |
| PB-03 | Interrupted at 400 | Resume without duplicates |
| PB-04 | New photo taken during run | Picked up next diff |
| PB-05 | Free phone storage | Offered only for verified items; hash re-checked before delete |
| PB-06 | Hub trashes file after backup | Phone does not mark as safe copy (re-diff) |
| PB-07 | Wi-Fi-only on, on mobile data | No upload |
| PB-08 | EXIF missing/corrupt | Falls back to mtime; no crash |
| PB-09 | Android OEM battery killer | Backup completes via foreground service or guided exemption |

### 3.6 Storage, health, integrity
| ID | Case | Expected |
|---|---|---|
| ST-01 | Existing-file scan on 100 GB messy disk | Accurate category sizes, no modifications |
| ST-02 | Import "Later" default | No files touched |
| ST-03 | Exact duplicates | Groups + reclaimable bytes correct |
| ST-04 | Resolve duplicates | Others go to trash; restorable |
| ST-05 | SMART caution/failing simulated | Alert + persistent banner + push |
| ST-06 | Flip a stored file byte | Scrub detects `hash_mismatch`, alerts, repairs from second copy |
| ST-07 | Second-copy run, unplug drive mid-run | Partial status; resumes; verified |
| ST-08 | Low space (<15%, <5%) | Warn then critical |
| ST-09 | Library moved to another drive | All paths valid; verified |

### 3.7 Remote, power, wake
| ID | Case | Expected |
|---|---|---|
| RM-01 | Trackpad/keyboard latency | p95 <60 ms LAN |
| RM-02 | Input without `remote` scope | 403 |
| RM-03 | Input while no user logged in / lock screen | Graceful "unavailable" message |
| RM-04 | Power: sleep/restart/shutdown | Executes after confirm; audited |
| RM-05 | WoL from sleep (supported NIC) | Wakes; documented per-model result |
| RM-06 | WoL from off, Wi-Fi only | Not promised; item queued |
| RM-07 | Remote session idle 60 s | Auto-ends |

### 3.8 Screen sharing (Phase 3)
| ID | Case | Expected |
|---|---|---|
| SS-01 | 1080p30 on L3 | Stable, <150 ms latency target |
| SS-02 | L1 weak laptop | Preset auto-downgraded or "Limited" message |
| SS-03 | Wi-Fi congestion | Adaptive bitrate; no Hub crash |
| SS-04 | Phone→Laptop cast | Fullscreen receiver, clean stop |

## 4. Performance benchmarks

| Metric | Target | Method |
|---|---|---|
| Idle RAM | <80 MB | Perf counters over 1 h idle |
| Idle CPU | <1% | Same |
| 2 GB over Gigabit (L3) | 20–30 s | `hh-tools bench` |
| 2 GB over Wi-Fi 5 (L2) | 1–1.5 min | Real phone |
| 2 GB over 802.11n (L1) | 2.5–7 min | Real phone |
| CPU during 2 GB transfer (L1) | <40% | Perf counter |
| 1,000-photo backup (≈4 GB) | Matches link throughput ±25% (small-file overhead) | Real phone |
| Thumbnail generation (L1) | ≥3 images/s idle-priority, doesn't block transfers | Instrumented |
| Pair time | <30 s median | Stopwatch × 10 |
| Startup to advertising | <10 s after boot | Event log |

Report all runs with link rate, RSSI, hardware, and Windows version. Track in `docs/benchmarks/` over time.

## 5. Reliability and soak
- **72 h soak** on L1: continuous mixed transfers + scrub + thumbs; no memory growth >10%, no handle leaks.
- **Power-cut test:** 200 randomized hard resets (VM or smart plug) during transfers/finalizes; 0 corrupt/orphaned files.
- **Sleep/resume cycles:** 100 cycles; service recovers, mDNS re-advertises.
- **Update/rollback:** upgrade N→N+1→rollback; DB migrations backward-safe via backups.

## 6. Compatibility
- Windows 10 22H2, Windows 11 23H2/24H2, x64 only (ARM later)
- Defender/SmartScreen behavior on fresh install (signed vs unsigned)
- Third-party AV (Quick Heal, Norton, McAfee common in India) interference with service/firewall
- Router matrix incl. common Indian ISP routers (Jio, Airtel, BSNL) for mDNS/isolation behavior

## 7. Usability testing
- 5 non-technical participants per round; tasks: install, pair, send a video, back up photos, find a photo, interpret "disk may be failing".
- Success = unaided completion; measure time and errors; record vocabulary users use ("my home computer" vs "server").
- Repeat after major UX changes; Hindi pass before wider pilot.

## 8. CI integration

| Stage | Runs |
|---|---|
| PR | fmt, clippy, unit, integration (fake-client), `cargo audit/deny`, Android unit tests |
| Nightly | Fault-injection suite, fuzz (short), perf regression vs baseline |
| Weekly | Soak on dedicated L1 machine, power-cut harness |
| Release | Full matrix, signed-build verification, installer install/upgrade/uninstall tests |

## 9. Bug severity

| Sev | Definition | Example | Policy |
|---|---|---|---|
| S0 | Data loss/corruption or security breach | Partial file visible; auth bypass | Stop-ship; fix + postmortem |
| S1 | Core flow broken | Can't pair, transfers fail | Fix before release |
| S2 | Degraded behavior | Wrong ETA, thumbs missing | Scheduled |
| S3 | Cosmetic | Copy typo | Backlog |

## 10. Exit gates

- **Phase 0:** All TR/QU/PA suites pass on L1–L3; benchmarks within target; zero S0/S1.
- **Phase 1:** + PB/ST/RM suites; 72 h soak; power-cut 0 failures; usability ≥80% unaided completion; external security review complete.
- **Phase 2/3/4:** Phase-specific suites (WoL matrix, SS suite, OS installer/update/rollback) added to gates.
