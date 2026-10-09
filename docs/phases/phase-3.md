# Phase 3 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Browse/search/sort/page files; select, rename, download, trash and restore them. Code complete, untested in BUILD.
- [~] Enforce purge eligibility and recover visibly from filesystem failures. Code complete, untested in BUILD.
- [~] Scan existing files and offer Keep / Import / Later, defaulting to Later; verify imported copies and preserve originals. Code complete, untested in BUILD; Windows copy and crash recovery remain unverified.
- [~] Exclude kept-in-place files from destructive Hub retention. Code complete, untested in BUILD; all storage mutations use `assert_mutable` to reject externally managed originals.
- [~] Move libraries with space checks, progress, verification and interruption recovery. Code complete, untested in BUILD; Windows switch and recovery remain unverified.
- [~] Review exact duplicates before cleanup to trash.
- [~] Suggest similar photos without automatic deletion.
- [~] Derive storage totals and reclaimable space from actual file state.
