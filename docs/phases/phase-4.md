# Phase 4 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Discover Android MediaStore photos/videos incrementally using the persistent Room queue. Code complete, untested in BUILD; Windows Hub and Android device validation remain.
- [~] Offer source approval, schedules, Wi-Fi/charging conditions, foreground progress and permission recovery. Code complete, untested in BUILD; Android permission recovery remains unverified on device.
- [~] Link backup items to completed transfers and mark verification only after durable finalization. Code complete, untested in BUILD; completed transfer ownership, queued hash and Hub bytes are checked before verification.
- [~] Recheck changed source content and missing, trashed or corrupt Hub copies. Code complete, untested in BUILD; Android requeues changed generations and Hub summary/cleanup rechecks downgrade unavailable copies.
- [~] Complete EXIF, thumbnails, Year/Month views, full viewers, zoom, sharing and details on Android/Windows. Code complete, untested in BUILD; Android plays verified originals and shares them, while Windows has server-filtered months, full viewer, zoom and share/save flow.
- [~] Preserve unsupported originals and show honest preview placeholders.
- [~] Require fresh Hub eligibility and matching local hashes before Android system deletion.
- [~] Protect eligible Hub copies during cleanup and record removal only after confirmed system results.
