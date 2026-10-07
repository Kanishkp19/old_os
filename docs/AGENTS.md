# AGENTS.md — Instructions for Coding Agents

Read this first. Then read `PRD.md`, `TRD.md`, `BACKEND_SCHEMA.md`, `API_SPEC.md`, `SECURITY.md` before writing code. Build in the order given by `IMPLEMENTATION_PLAN.md`. Do not skip ahead to later phases.

---

## 1. Project in one paragraph
Home Hub is a Windows background service (Rust) that turns an old laptop into a private LAN-only home cloud, with Android and macOS clients. Core: QR pairing with mTLS, chunked + resumable + BLAKE3-verified transfer, queue-and-send-later, then photo backup, gallery, disk health, remote control. The Linux "Home Hub OS" is Phase 4 and **out of scope until the validation gate**.

## 2. Hard rules (never violate)

1. **Never lose or corrupt user data.** Partial files must never appear in the library. Finalize = verify → fsync → atomic rename → DB commit.
2. **Never delete or move existing Windows user data** unless the user explicitly chose Import (copy by default) or approved a trash action.
3. **LAN-only.** No UPnP, no public listeners, no outbound calls except opt-in update checks. No telemetry by default.
4. **All routes default-deny** behind mTLS + scope check. Dashboard binds 127.0.0.1 only.
5. **Never trust client paths.** Canonicalize, jail to root, reject traversal and reserved Windows names.
6. **Verification gates deletion.** "Free phone storage" is only offered for items with `backup_items.status='verified'` and matching hashes.
7. **No secrets in logs** (tokens, keys, file contents).
8. **Resource budget:** idle <80 MB RAM, <1% CPU. No Electron in resident components.
9. **OS-specific code goes behind traits** (`PowerControl`, `InputControl`, `DiskHealth`, `HwAudit`, `ServiceHost`) so Phase 4 can port.
10. **Do not promise wake-from-off or universal hardware support** in code, UI, or docs. Queue is the guarantee.

## 3. Stack (fixed unless a doc is updated first)

| Area | Choice |
|---|---|
| Hub | Rust stable, tokio, axum/hyper, rustls (TLS 1.3 only), rusqlite or sqlx (SQLite WAL), blake3, mdns-sd, tracing, windows-service, tray-icon |
| Dashboard | Static Svelte/Vite bundle embedded in binary |
| Android | Kotlin, Compose, Hilt, Room, OkHttp (H2), WorkManager, CameraX + ML Kit |
| macOS | Swift/SwiftUI, URLSession, Network.framework |
| IDs/time | ULID; epoch ms UTC |
| Hash | BLAKE3 (4 MiB chunks + tree root) |

If you need to deviate, **update the relevant doc in the same PR** and explain why.

## 4. Conventions

### Rust
- `cargo fmt`, `cargo clippy -- -D warnings` must pass.
- No `unwrap()`/`expect()` outside tests and startup invariants; use typed errors (`thiserror`) mapped to API error codes in `API_SPEC.md §1`.
- Async everywhere on I/O; use `spawn_blocking` for hashing/disk-heavy work; keep hashing off the reactor.
- DB: one writer task or connection pool with `BEGIN IMMEDIATE` for writes; migrations are forward-only files in `hh-db/migrations/`.
- Each crate has a clear boundary (see README layout). No cross-crate cycles.
- Public API of each crate documented with doc comments + examples.

### Kotlin / Swift
- Unidirectional data flow (ViewModel + StateFlow / ObservableObject).
- No network on main thread. Queue persisted before any upload starts.
- Strings externalized (EN now, HI next). No hardcoded user-facing text.
- Follow `UI_UX_DESIGN.md` tokens and microcopy; avoid jargon words (sync, hash, mTLS, SMB, port) in user-facing text.

### Git
- Conventional commits (`feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `perf:`, `chore:`).
- Small PRs tied to a task ID from `IMPLEMENTATION_PLAN.md`.
- Schema or API changes must update `BACKEND_SCHEMA.md` / `API_SPEC.md` in the same PR.

## 5. Build order (summary)

**M0:** workspace, CI, DB migration framework, console-mode service, `/v1/info`.
**M1 (Phase 0):** CA + pairing → mTLS API → transfer engine (chunk/verify/resume/finalize) → Windows service + installer + tray → Android (discovery, pairing, share-sheet upload, queue) → macOS → fault-injection harness.
**M2 (Phase 1):** photo backup + gallery → file browser/import/dedupe → SMART + scrub + second copy → remote via session helper → hardening + pilot.

Stop and request human review at each milestone exit gate.

## 6. Implementation notes and gotchas

- **Windows Session 0:** the service cannot inject input or capture the desktop. Build `hh-session.exe` (per-user helper) + named-pipe IPC with restrictive ACLs early.
- **Atomic finalize:** `.part` file must be on the **same volume** as destination so rename is atomic. Preallocate; `FlushFileBuffers` before rename.
- **Resume truth source:** SQLite `transfer_chunks` + chunk hashes. On startup, reconcile `.part` files against DB; orphans older than TTL are deleted.
- **mTLS verifier:** custom rustls `ClientCertVerifier` that checks chain to Hub CA **and** consults the revocation set in memory (refresh on revoke event).
- **QR pairing:** client must verify Hub CA fingerprint before sending token. Never accept pairing over unverified TLS.
- **Android background:** use WorkManager + foreground service for long runs; test on OEM battery killers; never poll aggressively (use NsdManager + connectivity callbacks).
- **Thumbnails/scrub:** run at idle priority, yield to transfers, pause on low battery.
- **Windows filenames:** reject/sanitize `CON, PRN, AUX, NUL, COM1–9, LPT1–9`, trailing dots/spaces, `:` (ADS), and over-long paths (use `\\?\` prefix carefully).
- **HEIC:** don't assume WIC HEIF extension is installed; degrade to placeholder thumb.
- **Timekeeping:** store UTC ms; never trust client clocks for security decisions.

## 7. Definition of done
- [ ] Tests: unit + integration; fault-injection for any data-path change
- [ ] `cargo fmt/clippy/test` green; Android/macOS tests green
- [ ] Docs updated (schema, API, UX copy)
- [ ] Security checklist (`SECURITY.md §10`) ticked
- [ ] Idle resource budget not regressed (run `hh-tools bench idle`)
- [ ] Works on reference laptops L1–L3 (CI nightly or manual log attached)

## 8. Things to ask a human before doing
- Adding any dependency with network access, telemetry, or native code not in §3
- Changing wire protocol, schema semantics, or security model
- Anything touching deletion, trash purge, or import/move behavior
- Opening any listener beyond the ports in `TRD.md §4`
- Starting Phase 4 (OS) work

## 9. Quick commands

```bash
cd hub
cargo fmt --all && cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo run -p hh-service -- --console
cargo run -p hh-tools -- bench transfer --size 2GiB
cargo run -p hh-tools -- fake-client pair && cargo run -p hh-tools -- fake-client upload --file ./sample.bin
cargo audit && cargo deny check
```
