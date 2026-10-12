# Phase 4 — Active Slice: P4.6a (Signed A/B Update Bundles & RAUC Configuration)

## Goal
Implement the release update packaging and system slot configuration for signed A/B updates (§2.5, §5.3):
1. In `tools/image-builder/scripts/update-bundle.sh`: Create the release update bundler script supporting RAUC bundle manifest generation, compatible-device metadata (`homehub-x86_64`), versioning, signing key/cert validation, and reproducible bundle creation with dry-run support.
2. In `tools/image-builder/rootfs-overlay/etc/rauc/system.conf`: Define the target system's RAUC slot layout (`root_a`, `root_b`), GRUB bootloader integration, status file, and update keyring trust root location.
3. In `tools/image-builder/tests/update-bundle-test.sh`: Automated test verifying bundle argument parsing, dry-run manifest synthesis, certificate verification, and bundle structure.

## Files to Touch
- `tools/image-builder/scripts/update-bundle.sh` (NEW)
- `tools/image-builder/rootfs-overlay/etc/rauc/system.conf` (NEW)
- `tools/image-builder/tests/update-bundle-test.sh` (NEW)

## Targeted Acceptance Checks
- `bash -n tools/image-builder/scripts/update-bundle.sh`
- `bash -n tools/image-builder/tests/update-bundle-test.sh`
- `bash tools/image-builder/tests/update-bundle-test.sh`
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo check --manifest-path tools/image-builder/installer/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.6a READY FOR CODEX GENERATION
- Gate: G8 (Signed A/B, Rollback, and Measured Budgets)
- Blockers: None
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §2.5, §5.3](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)

