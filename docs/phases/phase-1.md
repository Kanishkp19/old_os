# Phase 1 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Verify the QR-pinned hub before Android/tooling token submission.
  - [x] Android server-purpose validation: reject a paired device's client-only certificate during pinned TLS verification. The focused test failed before the fix and the full Android debug unit suite passes afterward.
    - Done test `clientOnlyCertificateCannotImpersonateHomeServer`: synthetic server certificate passes, client-auth-only certificate from the same CA fails.
    - Done check `:app:testDebugUnitTest`: Android unit tests pass after the trust change.
  - [x] Rust tooling QR gate: a rogue CA fails the scanned fingerprint gate; local `hh-net` test passes. A live network rogue-hub test remains in validation.
    - Done test `rogue_hub_fingerprint_is_rejected`: a second Hub CA cannot satisfy the scanned Hub fingerprint; the matching CA can.
- [~] Enforce pairing expiry, single use, attempt limits, manual consent and secure client key storage.
  - [x] Pairing token and consent regression slice: expiry, persisted attempt limit and attempt counting failed first, then passed with the token-row checks; replay, consent and manual-code tests pass.
    - Done test `token_replay_after_pair_commit_is_rejected`: successful atomic device/token commit burns the token; replay cannot create another device.
    - Done test `wrong_token_attempt_limit_closes_window`: five wrong tokens persist five failed attempts and lock the window before certificate issuance.
    - Done test `persisted_attempt_limit_blocks_claim`: a token already locked in SQLite cannot be claimed from a still-open window.
    - Done test `consent_is_bound_to_exact_request`: unapproved and changed requests cannot issue a certificate; only the approved request proceeds.
    - Done test `manual_code_requires_approval`: a correct six-digit code cannot issue a certificate until the on-Hub request is approved.
    - Done test `manual_code_attempt_limit_closes_window`: three wrong six-digit codes close the window.
    - Done test `expired_token_row_blocks_claim`: an expired persisted token cannot be claimed even while an in-memory window exists.
- [~] Apply scope, status and ownership checks and revoke active API/event/remote/screen sessions promptly.
  - [x] Revocation regression slice: SQLite rejects current and staged identities after device removal; live API/SSE/remote session cutoff remains in validation.
    - Done test `revoked_device_rejects_current_and_pending_serials`: revocation blocks both current and staged renewal identities.
- [~] Persist certificate renewal and handle old certificates correctly.
  - [x] Renewal regression slice: SQLite keeps the old identity until new-certificate activation, then revokes it; real client retry remains in validation.
    - Done test `staged_renewal_keeps_old_identity_until_activation`: the old cert remains usable before activation; activation revokes it and enables the new cert.
- [~] Make chunk writes, verification, rename, database commit and response retries durable and synchronized.
- [~] Revalidate interrupted chunk state and make completion idempotent.
- [~] Handle changed sources, empty files, collisions, short writes, disk full and bounded concurrency.
- [~] Stream large downloads with Range support.
- [~] Pause new sharing, safely stop or pause active work, and keep owner administration available.
