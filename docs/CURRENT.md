# Milestone: Integration E2E Sandbox & Final Validation Ledger

## Goal
Finalize local technical validation before Windows hardware testing:
1. Run and verify the end-to-end sandbox runner `tools/e2e_sandbox.sh` to confirm loopback daemon boot, fake-client mTLS pairing, chunked upload/resume, and SQLite WAL database consistency.
2. Update `docs/WINDOWS_VALIDATION.md` with complete evidence for all local passes (Rust 70/70, Desktop 2/2 + 0 warnings, Android 18/18, Apple project generation, and all 5 items from FIXLIST.md resolved).
3. Mark external hardware/Windows-only checks explicitly as pending physical device validation.

## Files to Touch
- `tools/e2e_sandbox.sh` (if any loopback port/path adjustments are needed)
- `docs/WINDOWS_VALIDATION.md` (record complete verification ledger)

## Targeted Acceptance Checks
- `bash tools/e2e_sandbox.sh`
- `cargo check --manifest-path hub/Cargo.toml`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`

## Status
- Status: COMPLETE
- Blockers: None
- Evidence: `bash tools/e2e_sandbox.sh` passed 24/24 assertions (including 5/5 fault-injection checks, WAL mode, and SQLite integrity); `cargo check --manifest-path hub/Cargo.toml` passed; `npm --prefix desktop run check` found 0 errors and 0 warnings; `npm --prefix desktop test` passed 2/2.
- Pending external validation: Windows installer, WebView2, real capture/input, and physical Android/Apple client workflows remain open in the validation ledger.
- Reference: [docs/TEST_PLAN.md](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/TEST_PLAN.md), [docs/WINDOWS_VALIDATION.md](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/WINDOWS_VALIDATION.md)
