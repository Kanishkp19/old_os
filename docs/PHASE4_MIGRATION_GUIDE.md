# Phase 4 — Windows Home Hub to Home Hub OS Migration Guide

Status: proposed migration design, 11 October 2026. The export, installer and restore functions described here **must be implemented and validated before use**. They are not commands available in today's product. This planning document performs no disk changes, key export or migration.

## 1. What a successful migration preserves

Move the existing Windows Home Hub to a dedicated Linux Home Computer while keeping file contents, hub identity, paired-device trust, permission/revocation decisions, photo/transfer verification history, notes/playlists/bookmarks and applicable settings. Copy data from NTFS to ext4; never attempt an in-place NTFS-to-ext4 conversion. Keep the Rust backend, existing ports and API payloads. The repository has 14 backend crates and **13** hub SQL migrations, through `0013_scrub_rotation.sql`; preserve the actual schema and all current tables rather than the prompt's historical count of 11.

Success requires byte/hash verification, consistent databases, identity matching, and a real transfer from an already paired device. A green installer progress bar or matching file count alone is insufficient. Windows installer/WebView2/hardware checks have not been performed on this Mac, and this document does not imply otherwise.

## 2. User-facing choices and installer flow

```text
Try Home Computer
  → Check this laptop
  → Choose where to install
      [Keep Windows — recommended]   [Replace Windows]
  → Review disk, space, encryption and what will be preserved
  → Confirm the exact destination
  → Install and verify Home Computer
  → Restore your files and connected devices
  → Check your phone connects
  → Ready
```

Default to “Keep Windows” or a separate empty target drive. Show friendly disk name, capacity, connection type and a stable identifier suffix. Never choose a disk solely because it is `/dev/sda` or disk 0. Disable Continue if the source, export destination, installer media and target cannot be distinguished safely.

| Choice | Required preparation | Behavior | Recovery |
|---|---|---|---|
| Keep Windows on a separate drive | Verified export; empty target with enough space | Install OS there; preserve Windows disk and boot files | Select original drive in firmware; keep old service disabled until an explicit migration-back decision |
| Keep Windows on the same drive | Backup; Windows prepares unallocated space; clean shutdown; supported GPT/boot layout | Create Linux partitions only in verified free space; preserve Windows/system/recovery partitions and existing ESP contents | Boot Windows entry; never automatically replay stale hub state |
| Replace Windows | Full verified off-device migration bundle plus independent recovery copy; explicit erase confirmation | Erase only the chosen destination after final review; create OS A/B/data/recovery layout | Reinstall Windows from separate recovery media if desired; restore data from verified export |
| Try without installing | Bootable USB and compatible hardware | Demonstration/compatibility checks with temporary identity isolated from the real hub | Reboot; nothing is imported or changed automatically |

“Replace Windows” confirmation states: **“This erases Windows and all files on [disk name, capacity, identifier]. Your verified backup is on [other device].”** Require selecting the named disk and a final Erase and install action. Do not accept a backup stored on the disk being erased. The installer repeats device identity immediately before writing and aborts on a mismatch/hot-unplug.

For same-disk Keep Windows, perform shrink/preparation in Windows with its supported disk tools, not from the Linux installer. Require BitLocker recovery material and a clean, non-hibernated shutdown; explain that Fast Startup/hibernation must be disabled for migration preparation. If the filesystem is dirty, encrypted without usable recovery, hibernated, unsupported dynamic storage or an untested boot layout, stop with a concrete preparation instruction. Do not force-mount writable NTFS or automatically remove hibernation files.

The 15-minute installation goal applies to tested hardware, prepared free space, OS write/verification and initial setup. Copying a large photo library, resizing Windows, and resolving encryption recovery are timed and estimated separately. Show “Installing Home Computer” and “Copying your files” as distinct progress stages.

### 2.1 Try without installing — live session contract

```text
USB boot menu: [Try Home Computer] [Install] [Recovery]
  → Language + accessibility + touch/mouse choice
  → Compatibility checks (display, keyboard, touch, Wi-Fi, audio)
  → Trial Home: “Trying from USB — changes are lost when you restart”
       [Sample photos] [Sample files] [Notes] [Calculator]
       [Test this laptop] [Install Home Computer] [Power off]
  → Optional install handoff → full disk-selection/confirmation flow
```

Default USB boot selection is Try. Mount the signed live root read-only; keep writes, temporary owner settings, browser profiles and the disposable library in a size-limited RAM overlay. Use bundled clearly labelled sample content. Enumerate internal disks for identity/capacity only: do not automount, unlock, repair, import from or write to them. No persistent swap on internal storage. The installer writer and migration tool do not start in Try mode.

Display an always-visible Trial badge and a banner explaining that notes, downloads, preferences and network credentials disappear on reboot. Allocate at most min(512 MiB, 25% of detected RAM) for writable trial data; show usage and reject writes before exhaustion. Check the measured minimum usable RAM before launching the trial; insufficient memory offers diagnostics/power-off, not an unannounced internal-disk overlay. The trial is a distinct workload from the <200 MB installed idle gate and reports its memory honestly.

No real hub identity is imported, paired devices are not discovered/adopted, and the trial does not advertise a paired service or open LAN API/pairing listeners. Local sample apps use a disposable loopback-only instance; upload/phone pairing are disabled with “Available after installation.” Sample files are marked “Demo,” never presented as discovered personal content. Browser/YouTube become available only once the same production sandbox has passed; before that, the trial shows them as unavailable rather than granting them trusted access. Wi-Fi/internet requires explicit connection/use; captured credentials remain in RAM only.

“Test this laptop” records real display, input/posture, brightness, sound, network and suspend/resume outcomes with pass/fail/untested labels. Optional suspend test explains that Wi-Fi will reconnect; no existing production hub session is touched. The default report contains hardware IDs/results, not file names, personal content or credentials. “Save report” requires choosing removable storage and confirming the destination; it never silently writes to an internal disk. Do not expose a general file picker that bypasses that boundary.

“Install Home Computer” starts the separate installer after explicitly leaving Trial mode; nothing has been preselected for erasure. Carry only validated language/accessibility/input-mode preferences in a small versioned handoff, not demo identity, notes, certificates or credentials. Re-enumerate disks and repeat full preflight/confirmation. Offer export of trial notes to user-selected removable storage before leaving. Cancel before disk confirmation returns to Trial unchanged. Power off/restart warns that unsaved trial changes will be lost and releases the RAM overlay; no promise of forensic RAM erasure.

Implementation files: new `tools/image-builder/installer/src/trial.rs` (trial state/allowed actions), `tools/image-builder/rootfs-overlay/etc/systemd/system/homehub-trial.service`, `tools/image-builder/rootfs-overlay/etc/homehub/trial.json`, `tools/image-builder/trial/sample-manifest.json`, and `tools/image-builder/tests/live-trial.sh`; update planned `board/homehub/grub.cfg` for default Try and planned installer `main.rs`/`flow.rs` for explicit handoff. In the labwc baseline the native view is `hub/crates/hh-shell/src/trial.rs`; register it in the shell's `lib.rs`. All are future files, not implemented by this revision. Deliver initial trial infrastructure in P4.1 and acceptance in P4.5.

Acceptance on disposable VMs: hash internal disks/partition tables before and after boot, app editing, crash, suspend and power-off; prove unchanged with no automounts. Verify no LAN listener/advertisement, fresh temporary identity per boot, RAM exhaustion handling, English/Hindi/touch/keyboard accessibility, deliberate removable-report export, install cancellation and no demo identity in an installed system. Repeat no-write checks on reference hardware using disposable internal test disks before offering a household trial.

## 3. Preconditions and inventory

The Windows export tool must run on the **original Windows installation**, with the service's authorized identity/key access and an owner-approved export. A copied Windows folder is insufficient to decrypt machine-bound DPAPI secrets. Privileged export authorizes one operation; it does not expose keys to the ordinary desktop process.

Before export:

1. Record hub ID, friendly name, CA SHA-256 fingerprint, server certificate validity, current application version, actual migration ledger/schema version, library roots, second-copy roots and used/free bytes.
2. Enumerate devices, active/revoked certificate serials, renewal records, backup sources/items, active transfers/chunk records, finalized files, trash, albums, jobs, second-copy proofs and recovery journals. Include soft-deleted data required for restore; do not silently drop trash.
3. Locate the real configuration and identity files from runtime configuration. Defaults may be `%ProgramData%\HomeHub`, but a hardcoded default must not miss a moved library or data directory.
4. Enumerate private desktop stores per Windows owner. v1 imports the selected owner into a new owner UUID; additional stores are exported separately and left unopened, not merged into a single owner.
5. Audit NTFS reparse points/junctions/symlinks, case/Unicode collisions, ADS, EFS, compressed/sparse files, filenames rejected by current jailing rules, unreadable files and external roots. Estimate copy time and destination needs.
6. Require destination space for exported payload plus 10% working margin. Linux target must fit system slots/recovery plus restored data, staging and at least max(10% of planned data allocation, 5 GiB) free. The planner accounts for whether verified files can be atomically moved from same-filesystem staging without making a second full copy.
7. Require AC power or adequate battery; inhibit sleep for snapshot/export and installation. Keep the recovery key and export passphrase available outside both computers.

The preflight report gives counts and actionable exceptions. It does not claim unreadable or excluded files were backed up. The owner can abandon migration without altering the current service/library.

## 4. Consistent Windows export

### 4.1 Quiesce and snapshot

Show “Preparing your files. New transfers will wait.” Pause admission of new work, stop remote/capture sessions, finish or safely suspend active writes at a documented boundary, stop background import/scrub/move/second-copy writers, flush private note edits, and take an exclusive migration lock. Do not rely only on an ordinary sharing toggle to stop internal writers.

Take `hub.db` and private `desktop.db` snapshots using SQLite's backup API while writers are quiesced, or after a clean shutdown/checkpoint. Run `PRAGMA integrity_check` and `PRAGMA foreign_key_check` on the snapshots. Copying a live `hub.db` without its WAL can lose committed transactions. Do not copy unrelated `-shm` state into the restore and assume consistency.

Freeze the final exported state until cutover. For large libraries an optional initial copy can run earlier, but the final pass must recheck every changed file against the frozen DB snapshot, remove superseded manifest entries and recompute affected hashes. If export is cancelled, release the lock and explicitly resume Windows service writes; the incomplete bundle remains unimportable.

Preserve existing transfer IDs, source stamps, verified chunks, partial byte ranges, finalization journals and backup links. Complete/cancel existing file-operation and library-move jobs before export, or retain their checkpoints with a compatible importer. Never export a DB row claiming finalization while copying an unrelated partial file.

### 4.2 Portable bundle layout

The following is a logical layout inside the authenticated archive; plaintext secrets and filenames must not be written alongside it:

```text
header: magic HHM1, format version, KDF parameters/salt, archive ID
encrypted authenticated records:
  manifest.json             # schema/version/roots/files/chunks/checkpoint IDs
  identity/ca.cert.pem
  identity/ca.private       # decrypted only in memory on original Windows
  identity/server.cert.pem
  identity/server.private
  databases/hub.db
  private/<source-owner>/desktop.db
  config/service.json
  config/portable-preferences.json
  payload/<root-id>/<relative-path>
  partials/<transfer-id>/...
  hashes/manifest.blake3
  final-record              # archive completeness and record-count commitment
```

Design the new codec around a reviewed implementation of Argon2id passphrase derivation and XChaCha20-Poly1305 authenticated encryption, not an ad-hoc cipher. Proposed bounded format: 16-byte random salt; Argon2id 64 MiB, 3 iterations, parallelism 1; 24-byte unique nonce per record; maximum 4 MiB plaintext record. Authenticate archive ID, record index, type and length; an authenticated final record commits to the count and manifest hash to detect truncation, duplication and reordering. The parser caps all lengths/KDF costs before allocation and supports an explicitly versioned codec. Pin crypto library versions during implementation; subject the format to security review and test vectors before shipping.

Use streaming to bound memory; do not load a photo library into RAM. Resume an interrupted export by creating a fresh archive/key/salt and reusing previously verified **source copy checkpoints**, never by reusing an AEAD nonce/key combination. A complete encrypted bundle can be reread on interrupted import. The import checkpoint references archive ID, manifest hash and verified files. Wrong passphrase/tampering produces no committed identity or data.

Write encrypted output to a temporary name, fsync, verify by decrypting and checking all entries, then atomic rename plus parent fsync. If removable media requires splitting, each part is authenticated and ordered under one archive ID; final manifest commits to all part hashes. Require every part before restore. Never log passphrases, raw private keys, tokens, filenames or file contents; diagnostics use counts, IDs and reason codes. Memory clearing is best effort with suitable zeroizing buffers; no claim of perfect erasure from RAM or SSD flash.

### 4.3 Export completion

Show verified file count/bytes, excluded items with reason, archive location, CA fingerprint and verification timestamp. Require a successful independent bundle readback; Replace Windows additionally requires a separately verified recovery copy on a different failure domain. Store no unencrypted identity backup. Keep the original Windows data until Linux acceptance; do not “free space” by deleting it during export.

## 5. Data restore: NTFS to ext4

### 5.1 Mapping and filesystem semantics

| Windows item | Linux destination | Restore rule |
|---|---|---|
| Service state / `hub.db` | `/var/lib/homehub` on Data | Preserve hub ID/schema/relationships; directory service-owned, 0700 |
| Identity files | `/var/lib/homehub/{ca.key.enc,ca.cert.pem,server.key.enc,server.cert.pem}` | Use Linux key writer, not direct DPAPI blob copy; keys created 0600 atomically |
| Writable configuration | `/var/lib/homehub/config.json` | Merge portable settings with explicit Linux paths; `/etc/homehub` stays immutable defaults |
| Library root | `/srv/homehub` on Data | Preserve current derived library layout and root IDs; final and `.hh-tmp` on same filesystem |
| Additional roots | Explicit owner-selected mount by UUID | No guessed remapping of an unplugged drive; mark unavailable until verified |
| Private desktop state | `/var/lib/homehub-users/<owner-id>` | Owner-only 0700 directory and private DB; no service/global sharing |
| Logs and thumbnails | Bounded Linux log/cache paths | Logs excluded by default; valid thumbnails may copy, otherwise regenerate after acceptance |
| Second-copy disk | Explicit UUID-mounted path | Keep historical proof; revalidate mounted target before new runs/claims |

Preserve file IDs, original relative paths when valid, sizes, content BLAKE3 hashes, capture dates and media metadata. Absolute Windows paths must be remapped structurally, never with a global search-and-replace in database bytes. Enumerate path-bearing columns/settings from the source version: `storage_roots.path`, `transfers.tmp_path`, thumbnail paths, file-operation journals, library move/job payloads and second-copy configuration. Preserve source-device identifiers, content strings and note text unchanged.

Detect each destination path collision after normalization and current cross-platform filename validation. Conflicting files are never overwritten. Invalid or colliding entries block final acceptance until an explicit mapping is recorded; a suggested suffix includes the stable file ID. Preserve original-name metadata in the encrypted migration manifest and update only the relevant destination path references transactionally. Do not relax Windows-name rejection just because ext4 supports more names; companions still use the existing contract.

Do not follow NTFS reparse points or symlinks out of the allowed source roots. ADS are not silently converted into ordinary files; report and preserve them only through a declared separate export choice. EFS content must be decrypted by an authorized Windows user before export or marked blocked. Sparse/compressed files are copied as logical content and verified; storage estimate assumes expanded size unless sparse preservation is explicitly supported. Retain logical timestamps where supported, but NTFS ACLs/SIDs do not become Linux permission bits. Linux ownership is applied from the selected service/owner identities.

### 5.2 Staging and commit

1. Boot live installer, audit hardware/storage, and complete the selected disk flow. Verify all destination identity/size checks before writing partitions. Mount the Windows source read-only if it is needed.
2. Create an import transaction directory on Data, containing a small migration journal. Authenticate the complete bundle structure/manifest before extracting names. Reject absolute paths, `..`, symlinks, duplicate archive entries, excessive lengths and unexpected data sizes.
3. Decrypt/copy files into private staging on the destination filesystem with bounded memory, no executable permissions and no externally visible partially restored library. fsync each file at checkpoint boundaries.
4. Recompute existing per-chunk and tree-root BLAKE3 values using the same transfer engine, plus size/manifest verification. Resume only when the staged file identity, size and recorded hash still match; otherwise recopy that file. Never mark unknown bytes verified.
5. Restore DB snapshots into staging, transactionally rewrite approved root/path references and reset only stale runtime state. Expire pairing tokens and pending pairing attempts; clear dashboard session secrets; end remote/capture sessions. Pause scheduled writes and phone-cleanup actions until verification completes. Do not erase persistent revocations, device scopes, renewal history, file/chunk IDs or backup proof.
6. Reconcile partial transfers, finalization and operation journals against staged content. Preserve resumable transfer IDs/chunk truth and mark missing partials explicitly retryable under existing contracts. Never interpret a copied `.part` as a completed file. In-flight cleanup leases must not survive as fresh authority: require expiration/revalidation using the existing cleanup protocol before any device deletes local data.
7. Validate identity/certificate consistency, `integrity_check`, `foreign_key_check`, counts, root mapping and every exported file hash. Missing external roots are shown separately; do not report complete backup until resolved. Pause second-copy schedules until target identity/hash reconciliation succeeds.
8. Stop the service and hold the migration lock. Rename each verified staged tree/database into place under a durable journal with before/after records and parent fsync. Multiple directory renames are **not one atomic transaction**: a startup guard must finish or roll back the recorded sequence before the normal service can run. Do not use a single “done” flag to hide partial commit.
9. Publish a final import-complete marker only after installed paths, ownership, DB and identity checks pass. Start the service in validation mode with mutation/pairing admission restricted, perform acceptance, then enable normal traffic. Retry of the same archive/transaction is idempotent; reject unrelated overwrite of an already active hub.

Fresh identity generation is forbidden whenever an import marker or partial identity exists. The current `load_or_create` behavior must be hardened for this distinction. If recovery cannot establish a consistent imported identity, show Restore again/Recover from backup; do not quietly pair everyone to a new CA.

## 6. Certificate and pairing continuity

The existing Windows files are `ca.key.enc`, `ca.cert.pem`, `server.key.enc`, and `server.cert.pem`. Windows private-key files use a `DPA1` prefix and machine-scope DPAPI. Current Unix code instead reads/writes PEM with 0600 permissions; the `.enc` suffix alone does not mean encryption.

On original Windows, an authorized export operation decrypts keys through DPAPI, validates each key against its certificate, and places the key bytes only inside the authenticated encrypted bundle. Linux verifies the CA fingerprint against the export report, imports the same CA key and certificate, preserves `hub.id`, and keeps DB/disk certificate references consistent. Protect keys with restrictive ownership and optional full-data LUKS2 encryption. Explain that permissions alone do not protect a stolen unencrypted disk.

Device private keys stay on Android/Apple/other clients and are never exported from them. Preserve device cert serials, IDs, public keys, scopes, revocation records and renewal state. Reusing a CA without restoring revoked serials would re-enable removed devices and is unacceptable.

Keep the server leaf certificate/key if valid and appropriate for the restored endpoint. If SANs/name/validity require renewal, issue a replacement under the **same CA** and update DB and disk consistently. Before calling that seamless, test actual Android/macOS/iOS clients for CA versus leaf pinning; leaf-pinning incompatibility is a migration blocker or a documented trust-repair flow, not permission to disable verification. New LAN address discovery should occur through existing mDNS/UDP discovery; do not instruct users to accept an unverified replacement CA.

Invalidate one-time pairing tokens and local dashboard sessions. Create a fresh local dashboard credential at first Linux service start and never include it in user-facing migration output. Bootstrap owner/broker access via the new Linux session authorization; do not carry Windows ACLs or service account names across.

After cutover only **one** machine/OS may advertise or serve the migrated identity. The Windows exporter leaves the old service disabled after final handoff and records a migration-away marker. Dual boot does not justify reactivating the stale Windows copy. Resuming it with old revocation/transfer state risks conflicting writes and restored access for revoked devices.

## 7. Settings and private applications

| Category | Preserve | Adapt / reset |
|---|---|---|
| Hub identity | Friendly name, hub ID and trusted CA | Server leaf only if validated renewal needed |
| Library and storage | File/root IDs, relative organization, retention, second-copy intent | Paths/mount UUIDs and target validation; no auto-purge during restore |
| Backup | Source IDs, item hashes, verified states and schedule intent | Reconcile destination content before cleanup or “safe to free” claims |
| Device access | Active/revoked identities, scopes, renewal state | Active sessions/pairing tokens reset |
| Network | Friendly device/network preferences where portable | Obtain Wi-Fi secrets through Linux setup; do not blindly copy Windows WLAN secrets/IP leases |
| Updates | User opt-in preference | Do not carry Windows installer manifest/key into Linux OS update trust; ship signed OS trust root |
| Power/display | User preferences where meaningful | Re-evaluate sleep/lid/wake capability, display scale, brightness and audio device IDs |
| Privacy/language | English/Hindi, contrast, text size, reduced motion, notification preference | Per-owner OS defaults for new options; telemetry stays off |
| Notes | IDs, titles, text, timestamps, trash state | Owner path; flush before snapshot; no silent overwrite |
| Playlists/bookmarks | Existing record IDs/content, file-ID references, bookmark URLs | Mark absent media; revalidate imported URL policy |
| Browser/YouTube | Bookmarks; optional remembered public URLs | Do not migrate cookies, passwords, tokens, camera/mic/site grants or Windows WebView profiles; sign in again |
| Calculator | Current portable history preference if present | Cap to 100 items; do not synthesize missing history |
| Downloads | Completed files included only if explicitly inventoried | In-progress downloads marked interrupted; paths/grants rebuilt |

The current `local_export` format includes notes/playlists/bookmarks, including their trash state, but **does not export preferences, browser sessions or download paths**. It is useful as an independent private-app backup, not a complete machine migration. The new migration exporter snapshots `desktop.db` separately and sanitizes runtime/browser data according to the table above before Linux import. It must not imply that the existing Export private apps button exports CA keys or the hub library.

Existing `albums`/`album_items` rows migrate with `hub.db`; do not create new album tables or lose memberships because the dashboard lacks album routes. OS shell settings and notifications use owner-scoped versioned JSON outside the core schema. New local IPC fills OS/album control gaps without changing companion REST contracts.

## 8. Acceptance and cutover checklist

The installer emits a local redacted verification report with manifest ID, versions, counts, test results and timestamps. Treat any failed required check as “Migration needs attention.”

| Check | Required evidence |
|---|---|
| Data | Every included file has matching size and BLAKE3 chunk/root proof; no unexplained omission or overwrite; source unchanged |
| Databases | SQLite integrity and foreign-key checks pass; actual migration ledger unchanged; expected rows/relationships preserved |
| Identity | Hub ID and CA fingerprint match; CA/server keys match certs; expected device/revocation counts and serials retained |
| Companion continuity | Already paired Android, macOS and iOS client connect without insecure trust bypass; transfer upload/download and verification succeed |
| Revocation / renewal | A revoked test credential still fails; valid renewal under same CA succeeds; old renewal state cannot resurrect revoked access |
| Transfer continuity | Representative partial transfer resumes with correct chunk map; duplicate finalization does not duplicate files |
| Photo safety | Backup state references actual verified restored content; pending cleanup requires renewed verification; thumbnails render or regenerate |
| Apps/settings | Notes text/trash, playlists/bookmarks and language/accessibility preferences correct; missing media and unsupported hardware explicit |
| Local security | No imported dashboard token; browser cannot read CA/private DB/local service; owner lock blocks remote input/capture |
| Restart / update readiness | Two normal reboots preserve data and trust; simulated bad-slot rollback retains shared data on a disposable test image |
| Windows isolation | Old Windows hub service remains disabled; never two active copies of the same identity |

For an individual household, test every client platform they actually use and record untested ones; release QA must cover all supported companion platforms. “Keep Windows” data remains until the user later chooses a separate cleanup action. Recommend retaining a verified recovery copy through at least two successful reboots and a successful phone backup, then managing retention explicitly rather than automatic deletion.

## 9. Failures, recovery and return to Windows

| Failure | Behavior | Recovery |
|---|---|---|
| Wrong bundle passphrase / authentication failure | No identity activation or unverified extraction | Retry correct passphrase or choose intact verified bundle; never bypass authentication |
| Cannot decrypt original DPAPI keys | Export cannot preserve pairing | Return to original authorized Windows installation; if unrecoverable, explicit new-hub flow with re-pairing, not false continuity |
| NTFS dirty/hibernated/locked | No write or forced mount | Return to Windows, resolve shutdown/encryption state, rescan |
| Disk full / disconnect while copying | Stop at last fsynced checkpoint; original untouched | Free or choose storage through explicit action, remount by UUID, revalidate checkpoint, resume |
| Corrupt file/hash mismatch | Quarantine and report file ID; no verified status | Recopy from source/second copy; user resolves irrecoverable item before acceptance |
| Power loss during DB/path commit | Normal service blocked by migration journal | Recovery completes or rolls back staged renames; rerun identity/integrity checks |
| Identity mismatch / missing revocations | No pairing/transfer admission | Restore consistent DB+identity from the same snapshot; never mix bundles |
| Unsupported Wi-Fi/GPU | Do not advertise full compatibility | Ethernet/basic supported display only when tested, or return to Windows |
| Bad OS update after migration | Boot previous root slot | Preserve current Data; run shared-data compatibility checks, not old-data overwrite |

Return to Windows **before any Linux writes** can reactivate the original quiesced snapshot after Linux is fully disabled, with a check that no new device/revocation/transfer state exists. After Linux has accepted writes, the Windows copy is stale: require a new verified reverse-migration/export tool and compatibility check before reactivation. Reverse migration is outside Phase 4 v1; retain read-only access/export of new user files and the recovery bundle, and explain the limitation before cutover. Booting Windows for unrelated use is allowed while its Home Hub service remains disabled.

If keys are permanently lost, data recovery and trust recovery are different outcomes. Recover file content from a verified backup, create a new hub only through explicit setup, and re-pair devices. Never claim CA continuity without possession of the original CA private key.

## 10. Engineering delivery and focused verification

Implement export/import in new `hub/crates/hh-tools/src/migration.rs`, identity codec in `hh-auth/src/migration.rs`, copy/checkpoint/root remap in `hh-storage/src/migration.rs`; harden `hh-auth/src/ca.rs` and service startup. Use existing SQLite snapshot support and schema; register commands in `hh-tools/src/main.rs`. Build installer flow in `tools/image-builder/installer/src/{main,storage,flow,recovery}.rs`, with native OOBE migration status in `hub/crates/hh-shell/src/oobe.rs` (or the optional Smithay branch's `hh-compositor/src/oobe.rs`). In P4.5, add Windows owner-approved export launch/progress to `desktop/src-tauri/src/main.rs` and its Settings view, passing only redacted progress/results to JavaScript; collect export secrets in the trusted native flow. This document does not add those commands now.

Required fixtures: clean Windows DPAPI bundle; wrong owner/context; expired leaf; revoked and renewed devices; live-WAL snapshot; every supported schema version selected for migration; duplicate paths/reparse/ADS/EFS exclusions; full disk; corrupt/truncated/reordered archive; partial transfer and cleanup lease; power loss after every commit step; same/different disk Keep Windows; Replace Windows with independent backup; private notes and missing playlist media. Test on disposable images/fixtures first and original-Windows hardware for DPAPI/boot behavior. Never use the real library for fault injection.

Keep checks bounded to the active migration slice. New `tools/image-builder/tests/migration-roundtrip.sh` must compare manifests, schema, identities and client behavior, not merely exit codes. A Mac-only test cannot establish DPAPI or NTFS/firmware installer acceptance. Record blocked platform checks, and do not retry a failing command more than twice.

Repository basis: `docs/SECURITY.md` §4 (machine-scope DPAPI and optional passphrase backup), `docs/BACKEND_SCHEMA.md` identity/storage/transfer model plus actual migrations, `hub/crates/hh-auth/src/ca.rs` key formats, `desktop/src-tauri/src/store.rs` private export exclusions, existing storage journals and fixed client API contracts. This guide describes new work required to turn those foundations into a safe, reviewable migration product.
