# Phase 4 — Active Slice: P4.4d (Desktop App Registry & Local Command Bridge)

## Goal
Implement the desktop app authorization registry, map trusted Tauri windows to allowed commands, and bridge local album CRUD and backup summary commands to Tauri:
1. In `desktop/src-tauri/src/app_registry.rs`: Implement the controller-owned app registry mapping window label + origin + owner session + app ID to authorized commands per §4.4.
2. In `desktop/src-tauri/src/os_bridge.rs`: Implement Tauri commands for `albums_list`, `albums_create`, `albums_update`, `albums_delete`, `albums_items_list`, `albums_items_add`, `albums_items_remove`, and `backup_summary`.
3. In `desktop/src-tauri/src/main.rs`: Register `app_registry` and `os_bridge`, replace single-window `trusted()` checks with app registry validation, and register the new commands in the Tauri invoke handler.
4. In `desktop/src-tauri/capabilities/trusted-shell.json`: Authorize trusted app window labels while strictly denying remote content.

## Files to Touch
- `desktop/src-tauri/src/app_registry.rs` (NEW)
- `desktop/src-tauri/src/os_bridge.rs` (NEW)
- `desktop/src-tauri/src/main.rs` (EDIT)
- `desktop/src-tauri/capabilities/trusted-shell.json` (EDIT)

## Targeted Acceptance Checks
- `cargo check --manifest-path desktop/src-tauri/Cargo.toml`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`
- `cargo check --manifest-path hub/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.4c complete; P4.4d READY FOR CODEX GENERATION
- Gate: G6 (All Nine Apps Function & Local Data Invariants)
- Blockers: None
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.4, §5.1, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)



