# Phase 4 — Active Slice: P4.5b (Migration CLI Tool & Roundtrip Validation)

## Goal
Implement the migration CLI subcommands in `hh-tools` and an automated end-to-end migration roundtrip validation script (§4.3, §5.1, §5.2):
1. In `hub/crates/hh-tools/Cargo.toml`: Add dependencies on `hh-auth`, `hh-storage`, `hh-db`, and `rusqlite`.
2. In `hub/crates/hh-tools/src/migration.rs`: Implement CLI commands for `export`, `verify`, and `import` handling SQLite snapshots, identity bundle encryption/decryption, and storage manifest generation.
3. In `hub/crates/hh-tools/src/main.rs`: Register the `migration` subcommand in the CLI parser.
4. In `tools/image-builder/tests/migration-roundtrip.sh`: Implement an automated roundtrip validation test that creates a synthetic hub data directory, exports an encrypted bundle, verifies bundle integrity, imports to a new data directory, and confirms identity and data continuity.

## Files to Touch
- `hub/crates/hh-tools/Cargo.toml` (EDIT)
- `hub/crates/hh-tools/src/migration.rs` (NEW)
- `hub/crates/hh-tools/src/main.rs` (EDIT)
- `tools/image-builder/tests/migration-roundtrip.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-tools`
- `cargo clippy --manifest-path hub/Cargo.toml --workspace --all-targets -- -D warnings`
- `cargo fmt --all --manifest-path hub/Cargo.toml -- --check`
- `bash -n tools/image-builder/tests/migration-roundtrip.sh`
- `bash tools/image-builder/tests/migration-roundtrip.sh`
- `git diff --check`

## Status
- Status: P4.5b READY FOR CODEX GENERATION
- Gate: G7 (Installer, Migration, and Data Continuity Verification)
- Blockers: None
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.3, §5.1, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)


