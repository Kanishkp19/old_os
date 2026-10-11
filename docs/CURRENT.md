# Phase 4 — Active Slice: P4.3c (System Power Coordinator, Suspend/Resume mTLS Continuity & Local Assets Foundation)

## Goal
Implement system power management, connection shutdown/drain handles before sleep, wake revocation, and local OS assets foundation:
1. Implement `hub/crates/hh-service/src/power.rs` (system power coordinator, logind sleep inhibition, wake/lock revocation for remote input and screen sharing per §4.1).
2. Update `hub/crates/hh-service/src/main.rs` to wire the power coordinator and graceful sleep shutdown.
3. Update `hub/crates/hh-net/src/lib.rs` to track active mTLS connection shutdown handles to drain/close before sleep.
4. Create `tools/image-builder/tests/suspend-resume.sh` (validation test for sleep inhibition, wake revocation, and safe queue recovery).
5. Define `desktop/assets/os/manifest.json` and `desktop/assets/os/LICENSES.md` for nine apps and bundled fonts (Roboto, Noto Sans Devanagari).
6. Implement `desktop/scripts/check-os-assets.mjs` and add `"assets:check"` to `desktop/package.json`.

## Files to Touch
- `hub/crates/hh-service/src/power.rs` (NEW)
- `hub/crates/hh-service/src/main.rs` (EDIT)
- `hub/crates/hh-net/src/lib.rs` (EDIT)
- `tools/image-builder/tests/suspend-resume.sh` (NEW)
- `desktop/assets/os/manifest.json` (NEW)
- `desktop/assets/os/LICENSES.md` (NEW)
- `desktop/scripts/check-os-assets.mjs` (NEW)
- `desktop/package.json` (EDIT)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-service -p hh-net`
- `bash -n tools/image-builder/tests/suspend-resume.sh`
- `npm --prefix desktop run assets:check`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`

## Status
- Status: P4.3c READY FOR CODEX GENERATION
- Gate: G4 (Isolation, Lock Screen, Notifications & Capture Baseline)
- Blockers: Physical sleep/wake cycle tests require Linux hardware/QEMU guest host
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §3, §4.6, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md), [PHASE4_UI_UX_DESIGN.md §2.1, §4.6](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_UI_UX_DESIGN.md)
