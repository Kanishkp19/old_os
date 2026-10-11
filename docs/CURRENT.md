# Phase 4 — Active Slice: P4.3a (Native Shell Chrome: Notifications, Quick Settings, OOBE & Broker App Launch Extension)

## Goal
Implement the native shell overlay features (Notifications, Quick Settings, OOBE) and extend the local broker IPC contracts:
1. Extend `hub/crates/hh-core/src/local_ipc.rs` with `LocalOp::AppLaunch`, `LocalOp::NotificationsList`, `LocalOp::NotificationDismiss`, `LocalOp::NotificationClearAll`, `LocalOp::QuickSettingsSet` and typed DTOs.
2. Update `hub/crates/hh-shell/src/ipc.rs` to wire typed app launch, notification list/dismiss, and quick settings calls.
3. Implement `hub/crates/hh-shell/src/notifications.rs` (banners, notification center, DND toggle, bounded history <= 50, critical safety warnings cannot be dismissed to healthy).
4. Implement `hub/crates/hh-shell/src/quick_settings.rs` (Wi-Fi, audio slider/mute, brightness, session lock, power controls with pending/error states per §4.13).
5. Implement `hub/crates/hh-shell/src/oobe.rs` (language English/Hindi, touch/mouse mode toggle, Wi-Fi setup, owner credential, pairing QR code, resumable step checkpoints).
6. Update `hub/crates/hh-shell/src/lib.rs` and `src/main.rs` to wire the new modules.

## Files to Touch
- `hub/crates/hh-core/src/local_ipc.rs` (EDIT)
- `hub/crates/hh-shell/src/ipc.rs` (EDIT)
- `hub/crates/hh-shell/src/notifications.rs` (NEW)
- `hub/crates/hh-shell/src/quick_settings.rs` (NEW)
- `hub/crates/hh-shell/src/oobe.rs` (NEW)
- `hub/crates/hh-shell/src/lib.rs` (EDIT)
- `hub/crates/hh-shell/src/main.rs` (EDIT)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test --manifest-path hub/Cargo.toml -p hh-core -p hh-shell`
- `npm --prefix desktop run check`
- `npm --prefix desktop test`

## Status
- Status: P4.3a IN PROGRESS
- Gate: G4 (Isolation, Lock Screen, Notifications & Capture Baseline)
- Blockers: Wayland guest smoke and external PipeWire/portal capture validation remain pending Linux build host
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §5.2, §5.4, §6](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md), [PHASE4_UI_UX_DESIGN.md §2.2, §3, §4](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_UI_UX_DESIGN.md)
