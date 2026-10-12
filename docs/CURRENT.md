# Phase 4 — Active Slice: P4.4c (Nine App Views & Frontend State Extraction)

## Goal
Extract nine views from `desktop/src/App.svelte` into modular components under `desktop/src/apps/`, establish modular frontend state in `desktop/src/state/`, and connect local album CRUD:
1. Implement extracted views: `desktop/src/apps/{Photos,Files,Browser,YouTube,Music,Notes,Calculator,Devices,Settings}.svelte`.
2. Implement state modules: `desktop/src/state/{hub,private,music,notes}.js`.
3. Update `desktop/src/App.svelte` to act as an app host router using the extracted components.
4. Update `desktop/src/api.js` to expose local broker album CRUD and backup summary commands.

## Files to Touch
- `desktop/src/apps/Photos.svelte` (NEW)
- `desktop/src/apps/Files.svelte` (NEW)
- `desktop/src/apps/Browser.svelte` (NEW)
- `desktop/src/apps/YouTube.svelte` (NEW)
- `desktop/src/apps/Music.svelte` (NEW)
- `desktop/src/apps/Notes.svelte` (NEW)
- `desktop/src/apps/Calculator.svelte` (NEW)
- `desktop/src/apps/Devices.svelte` (NEW)
- `desktop/src/apps/Settings.svelte` (NEW)
- `desktop/src/state/hub.js` (NEW)
- `desktop/src/state/private.js` (NEW)
- `desktop/src/state/music.js` (NEW)
- `desktop/src/state/notes.js` (NEW)
- `desktop/src/App.svelte` (EDIT)
- `desktop/src/api.js` (EDIT)

## Targeted Acceptance Checks
- `npm --prefix desktop run check`
- `npm --prefix desktop test`
- `npm --prefix desktop run assets:check`
- `cargo check --manifest-path hub/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.4c frontend extraction implemented; targeted checks pass (desktop check: 0 errors, 0 warnings)
- Gate: G6 (All Nine Apps Function & Local Data Invariants)
- Blockers: Desktop Tauri command registry does not yet expose `albums_*` or `backup_summary`; album and backup UI need that bridge before G6 can be accepted. Windows functional checks remain pending on this Mac.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §4.4, §5.1, §5.2](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)


