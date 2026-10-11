# Phase 4 — Active Slice: P4.3b (Dedicated Session Lock Client `hh-lock`, Lock Isolation & Portal Capture Backend)

## Goal
Implement the dedicated fail-closed session-lock client (`hh-lock`), lock authorization, portal capture coordinator, and lock isolation test:
1. Register `crates/hh-lock` in `hub/Cargo.toml` and create `hub/crates/hh-lock/Cargo.toml`.
2. Implement `hub/crates/hh-lock/src/lib.rs` and `src/main.rs` (session-lock lifecycle, obscuring all surfaces, redacted status, fail-closed policy per §2.2).
3. Implement `hub/crates/hh-lock/src/auth.rs` (owner PIN/passphrase verification with rate limiting and exponential backoff).
4. Implement `hub/crates/hh-session/src/capture_labwc.rs` (PipeWire/portal capture backend, active capture grant tracking, automatic teardown before lock per §5.4).
5. Create `tools/image-builder/rootfs-overlay/etc/systemd/system/homehub-lock.service`.
6. Create `tools/image-builder/rootfs-overlay/etc/xdg/xdg-desktop-portal/homehub-portals.conf`.
7. Create automated validation test `tools/image-builder/tests/lock-isolation.sh`.

## Files to Touch
- `hub/Cargo.toml` (EDIT: register `hh-lock`)
- `hub/crates/hh-lock/Cargo.toml` (NEW)
- `hub/crates/hh-lock/src/lib.rs` (NEW)
- `hub/crates/hh-lock/src/main.rs` (NEW)
- `hub/crates/hh-lock/src/auth.rs` (NEW)
- `hub/crates/hh-session/src/capture_labwc.rs` (NEW)
- `hub/crates/hh-session/src/lib.rs` (EDIT)
- `tools/image-builder/rootfs-overlay/etc/systemd/system/homehub-lock.service` (NEW)
- `tools/image-builder/rootfs-overlay/etc/xdg/xdg-desktop-portal/homehub-portals.conf` (NEW)
- `tools/image-builder/tests/lock-isolation.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-lock -p hh-session`
- `bash -n tools/image-builder/tests/lock-isolation.sh`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`

## Status
- Status: P4.3b macOS checks passed; Linux runtime gate pending
- Gate: G4 (Isolation, Lock Screen, Notifications & Capture Baseline)
- Blockers: `hh-lock` has no compositor-backed session-lock surfaces or owner credential source; `capture_labwc` refuses grants until portal/PipeWire session and node handles are integrated. Wayland guest smoke and capture teardown validation require a Linux build host.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §5.4, §6, §7](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md), [PHASE4_UI_UX_DESIGN.md §2.2, §3, §4](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_UI_UX_DESIGN.md)
