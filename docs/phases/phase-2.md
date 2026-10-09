# Phase 2 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Deliver the Tauri/Svelte launcher and all 14 planned navigation destinations. Code complete, untested in BUILD; one route table drives sidebar and home tiles, all 14 destinations render in the native shell, and unavailable private functions have dashboard explanations.
- [~] Reuse navigation, dialogs, progress, alerts, empty states and actionable errors. Code complete, untested in BUILD; shared shell controls and confirm dialog cover routes, progress and empty states are reused, while a neutral working state and safe retry reload make failures actionable.
- [~] Guide setup through hub name, library, hardware, existing data and pairing. Code complete, untested in BUILD; five steps link library selection, show an on-demand hardware audit, scan existing folders with Later as default, and finish at phone pairing.
- [~] Persist settings in the service instead of startup-only defaults. Code complete, untested in BUILD; the owner settings API stores name, copy root, sharing, remote, screen, update, maintenance and wake choices in the Hub database and runtime paths read those settings.
- [~] Administer device names, permissions, removal, last seen and security activity.
- [~] Expose notifications, backup rules, privacy, updates and advanced diagnostics.
- [~] Complete keyboard navigation, labels, text scaling and English/Hindi coverage.
