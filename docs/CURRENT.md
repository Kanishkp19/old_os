# Phase 4 — Active Slice: P4.5c (Guarded USB Creation Scripts)

## Goal
Implement guarded, safety-verified USB creation scripts for Linux/macOS and Windows (§4.3, §5.3):
1. In `tools/image-builder/scripts/create-usb.sh`: Implement Linux/macOS host USB creator with block device enumeration, system/boot disk rejection, mandatory erase confirmation, bounded image writing, and readback hash verification.
2. In `tools/image-builder/scripts/create-usb.ps1`: Implement the matching Windows PowerShell workflow with disk enumeration, non-removable/system disk rejection, explicit confirmation, raw image block write, and readback verify.
3. In `tools/image-builder/tests/create-usb-test.sh`: Automated validation script verifying safety guards, argument validation, and mock dry-run verification.

## Files to Touch
- `tools/image-builder/scripts/create-usb.sh` (NEW)
- `tools/image-builder/scripts/create-usb.ps1` (NEW)
- `tools/image-builder/tests/create-usb-test.sh` (NEW)

## Targeted Acceptance Checks
- `bash -n tools/image-builder/scripts/create-usb.sh`
- `bash -n tools/image-builder/tests/create-usb-test.sh`
- `bash tools/image-builder/tests/create-usb-test.sh`
- `cargo check --manifest-path hub/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.5c IMPLEMENTED; host shell checks and cargo check passed
- Gate: G7 (Installer, Migration, and Data Continuity Verification)
- Blockers: Windows PowerShell execution and physical USB write/readback remain pending on Windows CI and representative hardware.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §5.3](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)
