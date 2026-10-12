# Phase 4 — Active Slice: P4.4a (Isolated Web Runner & Sandboxed Browser Controller Foundation)

## Goal
Implement the standalone unprivileged web-runner foundation, bounded IPC controller protocol, download quarantine manager, and browser isolation test:
1. Create `desktop/web-runner/Cargo.toml` (standalone unprivileged web runner crate).
2. Implement `desktop/web-runner/src/main.rs` (runner process entry point with `--app-id`, `--profile-dir`, `--socket`).
3. Implement `desktop/web-runner/src/control.rs` (bounded IPC controller protocol for tab lifecycle, origin sanitization, and shutdown).
4. Implement `desktop/web-runner/src/downloads.rs` (quarantine download manager ensuring untrusted downloads are isolated pending user confirmation).
5. Update `desktop/src-tauri/src/browser.rs` to support quarantine download inspection and runner controller models.
6. Create automated validation test `tools/image-builder/tests/browser-isolation.sh`.

## Files to Touch
- `desktop/web-runner/Cargo.toml` (NEW)
- `desktop/web-runner/src/main.rs` (NEW)
- `desktop/web-runner/src/control.rs` (NEW)
- `desktop/web-runner/src/downloads.rs` (NEW)
- `desktop/src-tauri/src/browser.rs` (EDIT)
- `tools/image-builder/tests/browser-isolation.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path desktop/web-runner/Cargo.toml`
- `cargo test --manifest-path desktop/web-runner/Cargo.toml`
- `cargo check --manifest-path hub/Cargo.toml`
- `bash -n tools/image-builder/tests/browser-isolation.sh`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`

## Status
- Status: P4.4a READY FOR CODEX GENERATION
- Gate: G5 (Full Nine-App Suite & Browser Sandbox Verification)
- Blockers: Linux namespace setup (bwrap/bubblewrap) and WebKitGTK runtime require a Linux host; macOS host validates protocol, quarantine state machines, and contracts.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.2, §4.5, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md), [PHASE4_SECURITY_ARCHITECTURE.md §6.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_SECURITY_ARCHITECTURE.md)

