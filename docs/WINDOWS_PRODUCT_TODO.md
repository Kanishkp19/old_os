# Windows Home Hub delivery checklist

`[ ]` not implemented, `[~]` in progress, `[x]` implemented. Implementation does not imply validation.

Scope: Windows product first. No Linux images, bootloaders, partitioning, OS installers or M6 implementation. Everyday apps are part of this release.

Branch: `implementation/windows-home-hub`. Remote: `Kanishkp19/old_os`.

Sequence: implement all phases → comprehensive tests → fixes → final regression → physical validation → household pilot. Local build and unit checks have begun after the implementation pass; they do not replace the acceptance matrix.

## Implementation

### Phase 0 — Baseline and delivery

- [x] Preserve source, executable, APK and online SQLite backup
- [x] Trace runtime to snapshot and record provenance
- [x] Establish implementation branch and excludes
- [x] Push implementation checkpoint `add54d1` to the dedicated branch
- [~] Repair syntax/dependencies/config/contract defects
- [ ] Completed phase commit and push recorded

### Phase 1 — Security and transfers

- [~] Pinned pairing, atomic consent/token limits, renewal and revocation
- [~] Durable chunk/finalization recovery and source identity
- [~] Authorization, streaming Range downloads and pause-sharing
- [x] Reconcile the existing transfer-owner middleware with a single 410 denial path and add a path-level regression test
- [ ] Completed phase commit and push recorded

### Phase 2 — Windows interface

- [~] Tauri/Svelte launcher and shared dashboard
- [~] Guided setup, persisted settings and device administration
- [~] Notifications, accessibility and English/Hindi
- [ ] Completed phase commit and push recorded

### Phase 3 — Files and import

- [~] Stable browsing, search/sort, rename/copy/move and trash recovery
- [~] Keep/Import/Later with verified copies and external-data protection
- [~] Durable jobs, library moves and duplicate review
- [ ] Completed phase commit and push recorded

### Phase 4 — Photos and phone cleanup

- [~] Android incremental MediaStore queue and scheduling
- [~] Backup linkage, gallery metadata/thumbnails and viewer
- [~] Fresh hash cleanup leases and confirmed system deletion
- [ ] Completed phase commit and push recorded

### Phase 5 — Protection and hardware

- [~] Verified per-file second copies and reconnection
- [x] Rebuild a corrupt existing second-copy file from a verified temporary copy; reject nested target paths
- [x] Clear failed per-file coverage and derive freshness from every live file's verification time
- [~] Scrub atomic repairs, low-space/health warnings
- [~] Hardware/network/wake capability reporting
- [ ] Completed phase commit and push recorded

### Phase 6 — Remote and screens

- [~] Permission-scoped remote input and authenticated bounded helper IPC
- [~] Real Windows capture/rendering and explicit video-only direction
- [~] Reliable session stop/revoke/disconnect and quality control
- [ ] Completed phase commit and push recorded

### Phase 7 — Companions

- [~] Android workflows and recovery states
- [x] Produce a local Android debug APK from the implementation branch
- [~] Reproducible Apple projects with secure identities and persistent queues
- [~] macOS browsing/relay/viewer and iOS share/photo backup
- [ ] Completed phase commit and push recorded

### Phase 8 — Everyday apps

- [~] Isolated native browser tabs, downloads, permissions and bookmarks
- [x] Remove all private browser-history rows, including trashed rows, when clearing website data
- [x] Persist only tab addresses and the YouTube isolation flag in the private user database; offer explicit restoration without carrying forward website grants
- [ ] Validate tab restoration, failed-restoration retry, and profile isolation with WebView2 on Windows
- [ ] Verify closed-tab WebView2 profile cleanup and browsing-data clearing on Windows
- [~] Isolated YouTube window and fallback
- [~] Private offline Notes/playlists, Music and safe Calculator
- [ ] Completed phase commit and push recorded

### Phase 9 — Installation and operations

- [~] Canonical Inno installer and all runtime components
- [~] Service/login/firewall/ACL/data-preserving upgrade and uninstall
- [~] Opt-in signed updates, recovery, bounded logs and scrubbed diagnostics
- [ ] Completed phase commit and push recorded

## Validation — local checks passed; acceptance pending

| Group | Status | Evidence required |
|---|---|---|
| Build/contracts | Partial | Rust workspace unit/doc tests, macOS Rust/Tauri checks, frontend and Android unit tests pass. Windows build, Apple build, migrations on upgrade, audits and packaging pending. |
| Transfers | Pending | Empty through 20 GB, concurrency, crash/disk-full/lost responses, source changes, relay |
| Backup/deletion | Pending | 1,000 mixed items, permissions/background limits, cancellation/hash changes/cleanup races |
| Storage | Pending | Import preserves originals, trash, repair failures, missing drives and interrupted move |
| Security | Pending | Rogue hub, token replay, ownership, renewal/revoke, IPC, path escapes, hostile content |
| UI/apps | Pending | All routes, accessibility/languages, persistence/export, browser permissions/downloads |
| Windows/companions | Pending | Physical install/reboot/upgrade/uninstall, remote/cast, Android/macOS/iOS |
| Performance/reliability | Pending | Service <80 MB/<1% CPU, 72h soak, 100 resumes, 200 disposable-data crashes |
| Household pilot | Pending | 20 households over four weeks; documented M5 metrics and zero data loss |

## Delivery records

- Baseline source commit: `78610dd`. Runtime snapshot: sibling `home-hub-baseline-20261008`, kept outside version control.
- Implementation checkpoint `add54d1` was pushed to `origin/implementation/windows-home-hub` on 2026-10-09. It is not a completed-phase or release claim.
- Transfer authorization slice `286a5f2` was pushed on 2026-10-09; it also updated the task and validation ledgers.
- Second-copy repair and coverage slice `7ba6648` was pushed on 2026-10-09.
- Mac-host builds and unit tests have run; no build has been deployed over the original demonstrated application.
- Signing and physical hardware results must be recorded when actually available.
- Failures and environment limitations: record in `WINDOWS_VALIDATION.md`, never mark unavailable checks passed.

## Detailed remaining gates from the approved plan

- [ ] Phase 0: reproduce a final Windows build from this branch and confirm the running baseline's source relationship on the user's Windows laptop.
- [~] Phase 1: finish rogue-hub, token replay/concurrency, renewal/revocation, transfer crash/disk-full/source-change, ownership, Range and pause-sharing cases.
- [~] Phase 2: finish guided setup, all launcher routes, persisted preferences, device security administration, notifications, accessibility and English/Hindi review.
- [~] Phase 3: finish Keep/Import/Later, pagination and actions, trash/purge recovery, duplicate suggestions, library-move crash recovery and accurate reclaimable-space calculations.
- [~] Phase 4: finish Android incremental backup, 1,000-item run, viewer/EXIF/unsupported-original behavior and cleanup-race verification.
- [~] Phase 5: finish physical disk health, missing-drive/reconnect, scrub failure recovery, second-copy drive identity, hardware/network capability and wake checks.
- [~] Phase 6: finish Windows helper/capture/encoder, Android/macOS rendering and cast, consent and reliable session teardown on real hardware.
- [~] Phase 7: finish Android recovery journeys and reproducible macOS/iOS builds, pairing, queues, browsing, relay, share extension and iOS backup limits.
- [~] Phase 8: finish browser/YouTube isolation and permissions, Music/Notes private persistence/import/export and calculator accessibility checks on Windows.
- [~] Phase 9: execute Inno installer, service/login/firewall/ACL upgrade/rollback/uninstall, signed update and diagnostics flows on Windows; prepare signatures when credentials exist.
- [ ] Final testing: execute the complete matrix in `TEST_PLAN.md`, fix failures, rerun final regression, and record unavailable hardware honestly.
- [ ] Release gate: signed artifacts where possible, physical Windows/Android validation, 72-hour soak, 100 resumes, 200 disposable-data crash runs, and the four-week 20-household pilot.

## Checkpoint 2026-10-08

The implementation pass now contains the Svelte desktop app, Android and Apple companion source, Windows helper/capture work, storage and transfer contracts, and an Inno packaging path. Local Rust, frontend, Tauri and Android checks pass as recorded in `WINDOWS_VALIDATION.md`. None of the phase acceptance criteria or release gates are complete: Windows installation and screen hardware, full Xcode builds, exhaustive fault tests, signing, soak, and the household pilot remain outstanding. The original working service and data were not modified.
