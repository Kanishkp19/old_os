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
- [~] Repair syntax/dependencies/config/contract defects
- [ ] Completed phase commit and push recorded

### Phase 1 — Security and transfers

- [~] Pinned pairing, atomic consent/token limits, renewal and revocation
- [~] Durable chunk/finalization recovery and source identity
- [~] Authorization, streaming Range downloads and pause-sharing
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
- [~] Reproducible Apple projects with secure identities and persistent queues
- [~] macOS browsing/relay/viewer and iOS share/photo backup
- [ ] Completed phase commit and push recorded

### Phase 8 — Everyday apps

- [~] Isolated native browser tabs, downloads, permissions and bookmarks
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
- Mac-host builds and unit tests have run; no build has been deployed over the original demonstrated application.
- Signing and physical hardware results must be recorded when actually available.
- Failures and environment limitations: record in `WINDOWS_VALIDATION.md`, never mark unavailable checks passed.

## Checkpoint 2026-10-08

The implementation pass now contains the Svelte desktop app, Android and Apple companion source, Windows helper/capture work, storage and transfer contracts, and an Inno packaging path. Local Rust, frontend, Tauri and Android checks pass as recorded in `WINDOWS_VALIDATION.md`. None of the phase acceptance criteria or release gates are complete: Windows installation and screen hardware, full Xcode builds, exhaustive fault tests, signing, soak, and the household pilot remain outstanding. The original working service and data were not modified.
