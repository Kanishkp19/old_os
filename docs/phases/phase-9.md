# Phase 9 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Package service, desktop, helper, tray, assets and runtime dependencies through canonical Inno Setup.
- [~] Configure service ACLs, data folders, firewall, startup/recovery and non-elevated user components.
- [~] Preserve data through upgrade, repair, rollback and uninstall; support keep-data choice.
- [~] Stage opt-in signed updates with database backup and failed-upgrade recovery.
- [~] Bound logs and export scrubbed diagnostics.
- [ ] Sign release artifacts when credentials are available; validate Windows installer lifecycle.
