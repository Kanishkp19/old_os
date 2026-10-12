# Phase 4 — Active Slice: P4.6c (OS Metrics Engine & Idle Budget Verification)

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
- Status: P4.6c READY FOR CODEX GENERATION
- Gate: G8 (Signed A/B, Rollback, and Measured Budgets)
- Blockers: None
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §2.1, §5.3, §8](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)

