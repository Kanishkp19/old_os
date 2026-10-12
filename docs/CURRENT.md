# Phase 4 — Active Slice: P4.4b (Local Broker Album CRUD & Backup Dispatch)

## Goal
Implement transactional album CRUD in `hh-db` using existing `albums` and `album_items` tables (§4.4.1), along with the local broker dispatcher for albums and backup summary in `hh-net`:
1. In `hub/crates/hh-db/src/queries.rs`: Add bounded transactional album operations (`albums_list`, `albums_create`, `albums_update`, `albums_delete`, `albums_items_list`, `albums_items_add`, `albums_items_remove`) and `backup_summary`. Exclude trashed or unavailable media from visible counts and covers.
2. In `hub/crates/hh-net/src/local_ipc.rs`: Implement the local broker request dispatcher mapping `LocalIpcRequest` to `LocalOp::Albums*` and `LocalOp::BackupSummary` operations, returning typed `LocalIpcResponse`.
3. In `hub/crates/hh-net/src/lib.rs`: Expose `pub mod local_ipc;`.
4. Add focused unit tests across `hh-db` and `hh-net` for duplicate requests, stale rename, non-existent media errors, hidden/restored media, empty album, paging ties, and album deletion preserving underlying file bytes.

## Files to Touch
- `hub/crates/hh-db/src/queries.rs` (EDIT)
- `hub/crates/hh-net/src/local_ipc.rs` (NEW)
- `hub/crates/hh-net/src/lib.rs` (EDIT)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-db -p hh-net`
- `cargo clippy --manifest-path hub/Cargo.toml --workspace --all-targets -- -D warnings`
- `cargo fmt --all --manifest-path hub/Cargo.toml -- --check`
- `git diff --check`

## Status
- Status: P4.4b READY FOR CODEX GENERATION
- Gate: G6 (All Nine Apps Function & Local Data Invariants)
- Blockers: None on macOS; fully validated via local SQLite memory tests and typed IPC dispatch.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.4.1, §5.1, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)

