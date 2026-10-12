# Phase 4 — Active Slice: P4.5a (Authenticated Identity & Storage Migration Foundation)

## Goal
Implement passphrase-encrypted identity migration bundles in `hh-auth` and verifiable storage migration manifests with BLAKE3 checkpointing in `hh-storage` (§4.3, §5.1, §5.2):
1. In `hub/crates/hh-auth/src/migration.rs`: Implement passphrase-wrapped `IdentityBundle` encryption/decryption using AEAD and KDF, CA key/cert pair validation, and paired device migration records.
2. In `hub/crates/hh-auth/src/lib.rs`: Expose `pub mod migration;`.
3. In `hub/crates/hh-storage/src/migration.rs`: Implement `StorageMigrationManifest`, BLAKE3 file copy verification, and resumable `MigrationCheckpoint` tracking.
4. In `hub/crates/hh-storage/src/lib.rs`: Expose `pub mod migration;`.
5. Add unit tests verifying: roundtrip encryption/decryption, wrong passphrase rejection, CA fingerprint and key-pair integrity checks, file copy BLAKE3 hash verification, and resumable migration checkpointing.

## Files to Touch
- `hub/crates/hh-auth/Cargo.toml` (EDIT)
- `hub/crates/hh-auth/src/migration.rs` (NEW)
- `hub/crates/hh-auth/src/lib.rs` (EDIT)
- `hub/crates/hh-storage/src/migration.rs` (NEW)
- `hub/crates/hh-storage/src/lib.rs` (EDIT)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-auth -p hh-storage`
- `cargo clippy --manifest-path hub/Cargo.toml --workspace --all-targets -- -D warnings`
- `cargo fmt --all --manifest-path hub/Cargo.toml -- --check`
- `git diff --check`

## Status
- Status: P4.5a COMPLETE on macOS; Windows and Ubuntu CI pending
- Gate: G7 (Installer, Migration, and Data Continuity Verification)
- Checks passed: `cargo check --manifest-path hub/Cargo.toml`; `cargo test --manifest-path hub/Cargo.toml -p hh-auth -p hh-storage`; `cargo clippy --manifest-path hub/Cargo.toml --workspace --all-targets -- -D warnings`; `cargo fmt --all --manifest-path hub/Cargo.toml -- --check`; `git diff --check`.
- Note: The specified `HH_ID_MIG_V1\0` magic is 13 bytes. `hh-core::Error::Integrity` and serde/equality derives for `hh-db::DeviceRow` required two narrow shared-type edits. A separately stored CA fingerprint is not part of the requested payload format; key/certificate matching is validated on export and import.
- Blockers: None on macOS; Ubuntu and Windows CI still pending.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.3, §5.1, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)

