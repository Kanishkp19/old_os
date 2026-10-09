# Phase 3 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Browse/search/sort/page files; select, rename, download, trash and restore them. Code complete, untested in BUILD.
- [~] Enforce purge eligibility and recover visibly from filesystem failures.
- [~] Scan existing files and offer Keep / Import / Later, defaulting to Later; verify imported copies and preserve originals.
- [~] Exclude kept-in-place files from destructive Hub retention.
- [~] Move libraries with space checks, progress, verification and interruption recovery.
- [~] Review exact duplicates before cleanup to trash.
- [~] Suggest similar photos without automatic deletion.
- [~] Derive storage totals and reclaimable space from actual file state.
