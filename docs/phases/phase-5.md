# Phase 5 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Collect Windows disk health and report Unknown where unsupported. Code complete, untested in BUILD; provider failures and stale snapshots become Unknown, with bounded Windows provider output and a fallback library-volume row.
- [~] Warn about low space, drive failure/removal, integrity and single-copy exposure. Code complete, untested in BUILD; measured conditions deduplicate and clear on recovery, while integrity failures remain visible for review.
- [~] Schedule throttled scrub and use verified temporary files for atomic repair. Code complete, untested in BUILD; scheduled batches rotate across files, yield for transfers and publish only hash-verified repair copies.
- [~] Select stable external drives, copy incrementally, schedule/reconnect and report freshness. Code complete, untested in BUILD; drive markers survive path changes, same-volume targets are rejected, interrupted runs stay partial, reconnect retries and dashboard freshness follows the connected drive.
- [~] Derive second-copy indicators per file; partial jobs must not imply full protection. Code complete, untested in BUILD; file responses show verified/needs-copy/disconnected from current drive records and disk presence, while aggregate full protection requires a completed run.
- [~] Preserve second-copy data independently from immediate library deletion. Code complete, untested in BUILD; second_copy_files has no files foreign-key cascade, and trash/purge touch only the library path, retaining verified external bytes and metadata.
- [~] Audit hardware, show compatibility, recover DHCP/discovery and expose supported hotspot controls.
- [~] Prefer Ethernet, attempt supported wake, and retain queued delivery when wake fails.
- [~] Remove unconditional public-network probes from core status paths.
