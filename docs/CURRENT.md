# Phase 4 — Active Slice: P4.5d (Native Live Installer Engine)

## Goal
Implement the native live installer engine (§4.3, §5.3) supporting hardware audit, dry run, target confirmation, raw image writing with readback verification, post-install boot configuration, and rollback recovery:
1. In `tools/image-builder/installer/Cargo.toml`: Register the `homehub-installer` binary and dependencies.
2. In `tools/image-builder/installer/src/storage.rs`: Disk enumeration, partition table layout, target validation, and raw block writing/verification.
3. In `tools/image-builder/installer/src/flow.rs`: Step-by-step installation state machine (Audit → TargetSelection → Confirmation → ImageWrite → PostInstall → Completion).
4. In `tools/image-builder/installer/src/recovery.rs`: Rollback on interrupted install, partition table backup/restore, and diagnostic reporting.
5. In `tools/image-builder/installer/src/main.rs`: CLI entry point supporting `--dry-run`, `--audit`, `--target <disk>`, `--mode <replace|keep-windows>`, and structured JSON output.
6. In `tools/image-builder/tests/installer-test.sh`: Automated test verifying dry run, hardware audit, target safeguards, and recovery simulation.

## Files to Touch
- `tools/image-builder/installer/Cargo.toml` (EDIT)
- `tools/image-builder/installer/src/main.rs` (NEW)
- `tools/image-builder/installer/src/storage.rs` (NEW)
- `tools/image-builder/installer/src/flow.rs` (NEW)
- `tools/image-builder/installer/src/recovery.rs` (NEW)
- `tools/image-builder/tests/installer-test.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path tools/image-builder/installer/Cargo.toml`
- `cargo test --manifest-path tools/image-builder/installer/Cargo.toml`
- `cargo clippy --manifest-path tools/image-builder/installer/Cargo.toml -- -D warnings`
- `cargo fmt --manifest-path tools/image-builder/installer/Cargo.toml -- --check`
- `bash -n tools/image-builder/tests/installer-test.sh`
- `bash tools/image-builder/tests/installer-test.sh`
- `cargo check --manifest-path hub/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.5d implemented; targeted macOS checks pass.
- Gate: G7 (Installer, Migration, and Data Continuity Verification)
- Remaining: Linux hardware/EFI write and Windows 2022 checks require their target environments. Keep Windows mode is explicitly rejected until a partition-preserving path exists. The hub check passes with an existing future-incompatibility warning for `block v0.1.6`.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.3, §5.3](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)
