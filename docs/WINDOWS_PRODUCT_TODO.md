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
- [x] Exclude singleton photos from similar-photo suggestion groups after a hash split
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
- [~] Reproducible Apple projects with secure identities and persistent queues
- [~] macOS browsing/relay/viewer and iOS share/photo backup
- [ ] Completed phase commit and push recorded

### Phase 8 — Everyday apps

- [~] Isolated native browser tabs, downloads, permissions and bookmarks
- [x] Remove all private browser-history rows, including trashed rows, when clearing website data
- [x] Persist only tab addresses and the YouTube isolation flag in the private user database; offer explicit restoration without carrying forward website grants
- [x] Resolve address-bar search terms to an isolated public search page while rejecting explicit unsafe/local URLs
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
| Build/contracts | Partial | Rust workspace unit/doc tests, macOS Rust/Tauri checks, frontend and Android unit tests pass; Android debug APK packages locally. Windows build, Apple build, migrations on upgrade, audits and release packaging pending. |
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
- Browser-history clearing slice `ec660bd` and private tab-session restoration slice `ce7cc9c` were pushed on 2026-10-09.
- The approved-plan task inventory was expanded and pushed in `31d5fde` on 2026-10-09.
- Mac-host builds and unit tests have run; no build has been deployed over the original demonstrated application.
- Signing and physical hardware results must be recorded when actually available.
- Failures and environment limitations: record in `WINDOWS_VALIDATION.md`, never mark unavailable checks passed.

## Detailed remaining gates from the approved plan

This inventory mirrors every implementation item in the approved plan. `[~]` means source or a partial path exists; it is not an acceptance result. The validation table above and `WINDOWS_VALIDATION.md` track tests separately.

### Phase 0 task inventory

- [x] Trace and preserve the running dashboard binary, configuration, database, source snapshot and user data.
- [x] Reconcile the existing GitHub history, create the dedicated implementation branch, and exclude local secrets/build products.
- [x] Record Windows scope, phase order, known defects, implementation TODOs and the acceptance matrix.
- [~] Repair baseline syntax, dependencies, configuration loading and API contract mismatches.
- [ ] Reproduce the final Windows build from this branch and confirm its relationship to the running baseline on the user's laptop.

### Phase 1 task inventory

- [~] Verify the QR-pinned hub before Android/tooling token submission.
- [~] Enforce pairing expiry, single use, attempt limits, manual consent and secure client key storage.
- [~] Apply scope, status and ownership checks and revoke active API/event/remote/screen sessions promptly.
- [~] Persist certificate renewal and handle old certificates correctly.
- [~] Make chunk writes, verification, rename, database commit and response retries durable and synchronized.
- [~] Revalidate interrupted chunk state and make completion idempotent.
- [~] Handle changed sources, empty files, collisions, short writes, disk full and bounded concurrency.
- [~] Stream large downloads with Range support.
- [~] Pause new sharing, safely stop or pause active work, and keep owner administration available.

### Phase 2 task inventory

- [~] Deliver the Tauri/Svelte launcher and all 14 planned navigation destinations.
- [~] Reuse navigation, dialogs, progress, alerts, empty states and actionable errors.
- [~] Guide setup through hub name, library, hardware, existing data and pairing.
- [~] Persist settings in the service instead of startup-only defaults.
- [~] Administer device names, permissions, removal, last seen and security activity.
- [~] Expose notifications, backup rules, privacy, updates and advanced diagnostics.
- [~] Complete keyboard navigation, labels, text scaling and English/Hindi coverage.

### Phase 3 task inventory

- [~] Browse/search/sort/page files; select, rename, download, trash and restore them.
- [~] Enforce purge eligibility and recover visibly from filesystem failures.
- [~] Scan existing files and offer Keep / Import / Later, defaulting to Later; verify imported copies and preserve originals.
- [~] Exclude kept-in-place files from destructive Hub retention.
- [~] Move libraries with space checks, progress, verification and interruption recovery.
- [~] Review exact duplicates before cleanup to trash.
- [~] Suggest similar photos without automatic deletion.
- [~] Derive storage totals and reclaimable space from actual file state.

### Phase 4 task inventory

- [~] Discover Android MediaStore photos/videos incrementally using the persistent Room queue.
- [~] Offer source approval, schedules, Wi-Fi/charging conditions, foreground progress and permission recovery.
- [~] Link backup items to completed transfers and mark verification only after durable finalization.
- [~] Recheck changed source content and missing, trashed or corrupt Hub copies.
- [~] Complete EXIF, thumbnails, Year/Month views, full viewers, zoom, sharing and details on Android/Windows.
- [~] Preserve unsupported originals and show honest preview placeholders.
- [~] Require fresh Hub eligibility and matching local hashes before Android system deletion.
- [~] Protect eligible Hub copies during cleanup and record removal only after confirmed system results.

### Phase 5 task inventory

- [~] Collect Windows disk health and report Unknown where unsupported.
- [~] Warn about low space, drive failure/removal, integrity and single-copy exposure.
- [~] Schedule throttled scrub and use verified temporary files for atomic repair.
- [~] Select stable external drives, copy incrementally, schedule/reconnect and report freshness.
- [~] Derive second-copy indicators per file; partial jobs must not imply full protection.
- [~] Preserve second-copy data independently from immediate library deletion.
- [~] Audit hardware, show compatibility, recover DHCP/discovery and expose supported hotspot controls.
- [~] Prefer Ethernet, attempt supported wake, and retain queued delivery when wake fails.
- [~] Remove unconditional public-network probes from core status paths.

### Phase 6 task inventory

- [~] Finish permission-scoped trackpad, keyboard, scroll, media keys and confirmed power actions.
- [~] Authenticate bounded helper IPC and fail closed on identity/permission errors.
- [~] Recover helper startup, shutdown, user-session changes and reconnection.
- [~] Capture real Windows screens, prefer hardware H.264 and fall back to software encoding.
- [~] Complete Android/macOS viewers and consented Android-to-Windows cast.
- [~] Render real receiver video with bounded buffering, quality presets and overload handling.
- [~] Stop sessions on disconnect, revocation, pause, logout and user action.

### Phase 7 task inventory

- [~] Complete Android navigation, downloads, history, notifications, permissions and recovery states.
- [~] Provide reproducible macOS/iOS Xcode projects and signing guidance.
- [~] Finish macOS Keychain pairing, persistent queue, drag/drop, Finder sharing, browsing, downloads and viewer.
- [~] Deliver phone-to-Mac hub-staged relay with verified recipient and delivery state.
- [~] Finish iOS pairing, App Group share queue, sending and foreground photo backup.
- [~] Continue iOS backup opportunistically in background, preserve originals and explain platform limits.

### Phase 8 task inventory

- [~] Browser address/search, tabs, navigation, bookmarks, session restore, downloads, permission prompts and clearing.
- [~] YouTube isolated window, playback/fullscreen/sign-in where supported and installed-browser fallback.
- [~] Music library playback, queue, seek, volume, repeat/shuffle and private playlists.
- [~] Notes offline autosave, search, rename, trash/restore and Markdown/text import/export.
- [~] Calculator arithmetic, percentages, parentheses, keyboard, history and safe expression parsing.
- [~] Integrate all five apps into the launcher and common window behavior.

### Phase 9 task inventory

- [~] Package service, desktop, helper, tray, assets and runtime dependencies through canonical Inno Setup.
- [~] Configure service ACLs, data folders, firewall, startup/recovery and non-elevated user components.
- [~] Preserve data through upgrade, repair, rollback and uninstall; support keep-data choice.
- [~] Stage opt-in signed updates with database backup and failed-upgrade recovery.
- [~] Bound logs and export scrubbed diagnostics.
- [ ] Sign release artifacts when credentials are available; validate Windows installer lifecycle.

### Comprehensive validation and release inventory

- [ ] Build/contracts: Rust format/lint/build/tests, frontend, Android, Apple, migrations, audits and Windows packages.
- [ ] Transfers: empty/small/2 GB/20 GB, Unicode/folders, concurrency, resume, disk full, relay and lost responses.
- [ ] Backup/deletion: 1,000 items, permission/background limits, local hash changes, cancelled cleanup and cleanup races.
- [ ] Storage: original preservation, trash/purge, duplicates, scrub failures, drive removal and interrupted moves.
- [ ] Security: rogue hub, replay, scope/ownership, renewal/revoke, path escapes, IPC, hostile web/uploaded content.
- [ ] UI/apps: every route/error, accessibility, languages, private data, browser/YouTube and process cleanup.
- [ ] Windows/companions: install/reboot/upgrade/uninstall, real remote/cast and Android/macOS/iOS workflows.
- [ ] Performance/reliability: service <80 MB and <1% idle CPU, 72-hour soak, 100 resumes and 200 disposable-data crashes.
- [ ] Fix all recorded failures, rerun affected checks and the full final regression suite.
- [ ] Produce signed artifacts where possible, physical compatibility results and setup/recovery instructions.
- [ ] Run the four-week 20-household pilot with backup usability, retention, satisfaction, zero-loss and resource results.
- [ ] Make a separate OS/M6 decision only after these gates; do not implement M6 in this branch.

## Checkpoint 2026-10-08

The implementation pass now contains the Svelte desktop app, Android and Apple companion source, Windows helper/capture work, storage and transfer contracts, and an Inno packaging path. Local Rust, frontend, Tauri and Android checks pass as recorded in `WINDOWS_VALIDATION.md`. None of the phase acceptance criteria or release gates are complete: Windows installation and screen hardware, full Xcode builds, exhaustive fault tests, signing, soak, and the household pilot remain outstanding. The original working service and data were not modified.
