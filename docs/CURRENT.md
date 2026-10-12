# Phase 4 — Active Slice: P4.6b (Boot Health Watchdog & A/B Rollback Probes)

## Goal
Implement the boot slot health watchdog and automated A/B rollback test probes (§2.5, §5.3):
1. In `tools/image-builder/rootfs-overlay/etc/systemd/system/homehub-health.service`: Systemd oneshot health service triggered after service and shell startup, conditioned on non-trial mode.
2. In `tools/image-builder/rootfs-overlay/usr/libexec/homehub-health`: System health monitor performing data filesystem check, DB readability verification, service readiness, input/shell check, 60-second stability window, and explicit slot mark-good.
3. In `tools/image-builder/tests/ab-rollback.sh`: Automated fault injection test suite verifying healthy mark-good flow, unhealthy boot refusal, crash detection during stability window, and GRUB boot-counter exhaustion rollback.

## Files to Touch
- `tools/image-builder/rootfs-overlay/etc/systemd/system/homehub-health.service` (NEW)
- `tools/image-builder/rootfs-overlay/usr/libexec/homehub-health` (NEW)
- `tools/image-builder/tests/ab-rollback.sh` (NEW)

## Targeted Acceptance Checks
- `bash -n tools/image-builder/rootfs-overlay/usr/libexec/homehub-health`
- `bash -n tools/image-builder/tests/ab-rollback.sh`
- `bash tools/image-builder/tests/ab-rollback.sh`
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo check --manifest-path tools/image-builder/installer/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.6b COMPLETE — targeted acceptance checks passed on macOS
- Gate: G8 (Signed A/B, Rollback, and Measured Budgets)
- Blockers: None
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §2.5, §5.3](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)
