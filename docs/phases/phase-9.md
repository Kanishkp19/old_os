# Phase 9 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [x] Package service, desktop, helper, tray, assets and runtime dependencies through canonical Inno Setup. Code complete in BUILD.
- [x] Configure service ACLs, data folders, firewall, startup/recovery and non-elevated user components. Code complete in BUILD.
- [x] Preserve data through upgrade, repair, rollback and uninstall; support keep-data choice. Code complete in BUILD.
- [x] Stage opt-in signed updates with database backup and failed-upgrade recovery. Code complete in BUILD.
- [x] Bound logs and export scrubbed diagnostics. Code complete in BUILD; service and dashboard diagnostics scrub all IDs, tokens, paths and file contents.
- [ ] Sign release artifacts when credentials are available; validate Windows installer lifecycle.
