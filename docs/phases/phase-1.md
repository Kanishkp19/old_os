# Phase 1 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Verify the QR-pinned hub before Android/tooling token submission.
- [~] Enforce pairing expiry, single use, attempt limits, manual consent and secure client key storage.
- [~] Apply scope, status and ownership checks and revoke active API/event/remote/screen sessions promptly.
- [~] Persist certificate renewal and handle old certificates correctly.
- [~] Make chunk writes, verification, rename, database commit and response retries durable and synchronized.
- [~] Revalidate interrupted chunk state and make completion idempotent.
- [~] Handle changed sources, empty files, collisions, short writes, disk full and bounded concurrency.
- [~] Stream large downloads with Range support.
- [~] Pause new sharing, safely stop or pause active work, and keep owner administration available.
