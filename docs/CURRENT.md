# Active Slice — Premium Desktop Shell

## Goal
Replace the desktop admin layout with a lightweight OS-style shell, preserving all nine apps, translations, native commands, and state stores.

## Files to Touch
- `desktop/src/App.svelte`
- `desktop/src/style.css`
- `desktop/src/shell/{StatusBar,Dock,Window,QuickSettings,Icon}.svelte`
- `desktop/src/shell/{labels,windows}.js`
- `desktop/tests/windows.test.mjs`
- `docs/CURRENT.md`

## Targeted Acceptance Checks
- `npm --prefix desktop run check`
- `npm --prefix desktop test`
- `git diff --check`

## Status
- IMPLEMENTED (2026-10-12). Svelte check: 0 errors / 0 warnings; frontend tests: 5 passed; git diff whitespace check passed.
- Browser smoke checks with mocked Hub responses: nine app frames; calculator; minimize/restore/close; pointer drag, keyboard move/resize; per-app search retention; themes; drawer focus restoration; Hindi; clean-profile startup; 1440/1366/390/320px window bounds. No browser runtime errors.
- CSS-only wallpaper and icons; no new dependencies. API module, state stores, existing translations, native code, and all nine app component files unchanged.
- Native brightness and secure lock have no existing frontend command. Brightness is unavailable; `hh-lock` is an event request, not a security boundary or confirmed lock. App media volume uses HTML media controls.
- Real native app, Hindi rendering, and hardware validation remain pending.

## Previous Slice
### Phase 4 — Completed Slice: P4.6c (OS Metrics Engine & Idle Budget Verification)

## Goal
Implement the OS metrics measurement engine and idle budget test suite (§2.1, §5.3, §8, Gate G8):
1. In `hub/crates/hh-tools/src/os_metrics.rs`: Linux procfs/cgroups sampling for whole-system memory (RSS/PSS, MemTotal, MemAvailable), whole-system CPU, service process-tree usage, and budget thresholds (<200 MB system RAM, <3% system CPU, <80 MB service RAM, <1% service CPU), with macOS test/mock fallbacks.
2. In `hub/crates/hh-tools/src/main.rs`: Register `os-metrics` subcommand in `hh-tools` supporting `--duration`, `--service-name`, `--json`, and mock procfs override.
3. In `tools/image-builder/tests/idle-budget.sh`: Automated test verifying repeatable metric collection, budget boundary evaluation, and fault reporting on budget violations.

## Files to Touch
- `hub/crates/hh-tools/src/os_metrics.rs` (NEW)
- `hub/crates/hh-tools/src/main.rs` (EDIT)
- `tools/image-builder/tests/idle-budget.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test -p hh-tools`
- `cargo clippy -p hh-tools -- -D warnings`
- `bash -n tools/image-builder/tests/idle-budget.sh`
- `bash tools/image-builder/tests/idle-budget.sh`
- `cargo check --manifest-path tools/image-builder/installer/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.6c IMPLEMENTED; targeted checks passed on macOS (2026-10-12)
- Gate: G8 (Signed A/B, Rollback, and Measured Budgets)
- Blockers: Live Linux boot-duration measurement and Windows/Ubuntu CI remain pending; mock procfs verifies budget enforcement. `cargo check` reports an existing future-compatibility notice for `block` v0.1.6.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §2.1, §5.3, §8](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)
